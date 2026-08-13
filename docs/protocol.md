# Native Bridge 通信协议

协议版本：`rm-aim-sim/1`

Native Bridge 的设计目标是保持简单、跨语言，并且与具体队伍的自瞄框架解耦。

## 1. 云台控制命令：UDP JSON

仿真器监听配置项 `network.command_bind` 指定的地址，默认值为：

```text
127.0.0.1:39000
```

数据包格式：

```json
{
  "yaw_deg": 12.5,
  "pitch_deg": -4.2,
  "fire": true
}
```

字段语义：

- `yaw_deg` 和 `pitch_deg` 是**绝对云台目标角度**，不是增量命令；
- 角度单位为度；
- 仿真器会按照配置中的 yaw / pitch 限位以及云台最大角速度执行运动约束；
- `fire=true` 表示持续给出开火建议，真实发射频率仍由弹丸发射冷却时间决定；
- 如果大约 350 ms 内没有继续收到有效命令，则网络开火许可会失效，并回到人工控制逻辑。

当前几何正负号约定：

- 零位姿时枪口朝向世界坐标 `-Z`；
- pitch 为正时，枪口向上旋转；
- yaw 为正时，采用 Bevy 绕 `+Y` 轴的正方向旋转；从初始 `-Z` 朝向观察，会向 `-X` 方向转动。

如果你的下位机或自瞄项目采用相反的 yaw 正方向，建议在适配器中对 yaw 取反，而不是修改仿真世界本身的坐标约定。

## 2. 遥测数据：UDP JSON

仿真器向 `network.telemetry_target` 指定的地址发送遥测数据，默认值为：

```text
127.0.0.1:39001
```

数据结构示例：

```json
{
  "protocol": "rm-aim-sim/1",
  "timestamp_ns": 1786610000000000000,
  "gimbal_yaw_deg": 12.1,
  "gimbal_pitch_deg": -4.0,
  "gimbal_pose": {
    "translation_m": [0.0, 1.1, 0.0],
    "quaternion_xyzw": [0.0, 0.0, 0.0, 1.0]
  },
  "muzzle_pose": {
    "translation_m": [0.0, 1.1, -0.55],
    "quaternion_xyzw": [0.0, 0.0, 0.0, 1.0]
  },
  "camera_pose": {
    "translation_m": [0.0, 1.145, -0.1],
    "quaternion_xyzw": [0.0, 0.0, 0.0, 1.0]
  },
  "camera_info": {
    "width": 1280,
    "height": 720,
    "fx": 1108.5,
    "fy": 1108.5,
    "cx": 640.0,
    "cy": 360.0
  },
  "target_hp": 470.0,
  "target_max_hp": 500.0,
  "target_rpm": 120.0,
  "target_translation_speed_mps": 2.0,
  "shots": 25,
  "hits": 13,
  "hit_rate_pct": 52.0,
  "total_damage": 130.0,
  "rolling_dps": 40.0
}
```

说明：

- `timestamp_ns` 为 Unix / 系统时钟纳秒时间戳；
- 所有平移量单位均为米，使用世界坐标系；
- 四元数排列顺序为 `[x, y, z, w]`；
- 核心模型中的相机视为理想针孔相机；
- 当前畸变系数默认为 0，除非未来显式加入带畸变的相机模型。

## 3. 相机图像流：TCP 分帧 JPEG

仿真器监听 `network.camera_bind` 指定的地址，默认值为：

```text
127.0.0.1:39002
```

客户端建立 TCP 连接后，需要循环读取：

```text
40 字节大端序帧头
JPEG payload
40 字节大端序帧头
JPEG payload
...
```

帧头格式如下：

| 偏移 | 字节数 | 类型 | 含义 |
|---:|---:|---|---|
| 0 | 8 | bytes | ASCII 魔数 `AIMSIM01` |
| 8 | 8 | `u64` | 帧 ID |
| 16 | 8 | `u64` | 图像采集时间戳，单位 ns |
| 24 | 4 | `u32` | 图像宽度 |
| 28 | 4 | `u32` | 图像高度 |
| 32 | 4 | `u32` | JPEG payload 长度 |
| 36 | 4 | `u32` | 保留字段，当前固定为 0 |

Python 对应格式：

```python
struct.Struct("!8sQQIIII")
```

帧头之后紧跟 `jpeg_len` 字节的完整 JPEG 图像数据。

## 4. 时间戳约定

相机帧与遥测数据都携带系统时钟纳秒时间戳，因此适配器可以把两类数据放入同一个时间域中进行对齐。

如果需要研究视觉链路延迟，建议在 Detector、Tracker、Predictor、Planner 等模块内部继续记录各自的处理时间戳，并保留仿真器提供的原始采集时间。

不要用网络接收时刻替换仿真器的图像采集时间戳，否则会把网络传输延迟错误地混入传感器时间。

## 5. 适配器设计原则

队伍特有的通信协议转换应放在 Rust 仿真核心之外。

例如：

```text
Native Bridge <-> ROS 2 适配器 <-> 你的 ROS 2 自瞄项目
Native Bridge <-> Talos 适配器  <-> 共享内存自瞄项目
Native Bridge <-> C++ 客户端    <-> 已有非 ROS 自瞄链路
```

这样可以在不修改仿真物理、相机模型与统计逻辑的情况下，使用同一套 Benchmark 比较多个不同的自瞄代码库。
