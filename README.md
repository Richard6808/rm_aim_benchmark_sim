# RoboMaster 自瞄闭环仿真与 Benchmark 工具

这是一个**完全独立**的 RoboMaster 四装甲板自瞄闭环仿真器与回归测试工具。

本项目**不依赖、不修改、不引用** `Blackjack200/bevy_robomaster_simulator` 的源码、资源文件或工程目录。此前的 Daedalus 项目仅作为闭环仿真思路参考。本项目中的靶车几何、射击端几何、相机安装位置、弹丸物理、网络通信、命中判定、伤害统计和 Benchmark 逻辑全部独立实现。

## 1. 仿真内容

```text
                    外部自瞄程序
                         │
       相机 JPEG  ───────┼──────► Detector / PnP / Tracker
       云台遥测    ───────┼──────► 时间对齐 / 状态估计
                         │
                         ◄─────── yaw / pitch / fire
                         │
                ┌────────┴────────┐
                │   射击机器人     │
                │   云台           │
                │   相机           │  相机位于枪管正上方
                │   枪管 / 枪口    │
                └────────┬────────┘
                         │ 17 mm 荧光弹丸
                         ▼
                ┌─────────────────┐
                │    Armor 0      │
                │                 │
         Armor 3│        C        │Armor 1
                │                 │
                │    Armor 2      │
                └─────────────────┘
                         │
                    命中 / HP / DPS
```

靶车只保留 **4 块装甲板**，不存在隐藏底盘碰撞体，因此弹丸必须真正撞击装甲板才算命中。

射击机器人在画面中只显示 **云台、枪管、枪口和相机**，但内部保留一个不可见的逻辑底盘根节点，用于 WASD 平移。

主窗口显示的画面与发送给外部自瞄程序的离屏相机画面使用同一个相机视角。

## 2. 主要功能

- 程序化生成四装甲板靶车，不依赖 GLB 或外部模型资源。
- 前后装甲与左右装甲的半径可以独立配置。
- 靶车支持按 RPM 旋转。
- 靶车支持多种平移轨迹：
  - 静止
  - X 方向正弦运动
  - Z 方向正弦运动
  - 椭圆运动
  - 8 字运动
- 可调靶车平移速度以及 X/Z 运动空间。
- 可调靶车最大 HP、单发伤害以及死亡后是否停止运动。
- 使用直径 **17 mm** 的绿色发光弹丸。
- 弹丸支持重力和连续碰撞检测，降低高速小弹丸穿透装甲板的概率。
- 相机安装在枪管轴线正上方。
- 提供项目无关的 Native Bridge：
  - TCP 输出 JPEG 相机画面；
  - UDP 输出云台、枪口、相机位姿以及实时统计；
  - UDP 接收 `yaw / pitch / fire` 控制命令。
- 提供独立的 ROS 2 适配器，不让 ROS 2 依赖侵入 Rust 仿真核心。
- 提供 egui 控制面板。
- 支持 RoboMaster 风格人工操作：
  - `W/A/S/D` 控制射击机器人底盘平移；
  - 鼠标控制云台；
  - 按住鼠标右键启用外部自瞄；
  - 按住鼠标左键表示操作手允许开火；
  - 只有自瞄同时给出 `fire=true` 时才真正发射弹丸；
  - 人工测试与自动测试使用同一套命中率、DPS、HP 和弹丸物理统计。
- 支持自动遍历 `距离 × RPM × 平移速度` 的 Benchmark。
- Benchmark 使用 `Warmup -> Running -> Drain` 状态机。
- Benchmark 不会绕过自瞄程序强制开火，最终开火仍由外部自瞄的 `fire` 决定。
- 自动导出每次试验、工况聚合、RPM 性能退化以及完整配置快照 CSV。
- 提供 Python 绘图工具，可生成命中率、DPS、击杀成功率、平均击杀时间和 RPM 退化曲线。

## 3. 技术栈

项目当前使用：

- Rust 2024 Edition
- Bevy `0.19`
- Avian3D `0.7`
- bevy_egui `0.41.1`

### Ubuntu 编译依赖

Ubuntu 22.04 / 24.04 可先安装：

```bash
sudo apt update
sudo apt install -y \
  build-essential pkg-config libasound2-dev libudev-dev \
  libx11-dev libxi-dev libgl1-mesa-dev \
  libxcb-render0-dev libxcb-shape0-dev libxcb-xfixes0-dev
```

然后使用 rustup 安装稳定版 Rust。

### Windows环境下运行
```bash
winget install Rustlang.Rustup
```

启动运行：

```bash
cargo run --release -- --config config/default.toml
```

## 4. 快速通信测试

另开一个终端运行：

```bash
python3 scripts/native_client.py --preview
```

如果还希望让测试客户端发送一组演示云台命令：

```bash
python3 scripts/native_client.py --preview --demo-command
```

在人工操作模式下，需要：

```text
按住右键 RMB
    ↓
启用外部自瞄控制

按住左键 LMB
    ↓
允许自瞄开火
```

演示客户端会周期性切换 `fire` 建议。只有所有开火条件同时满足时，仿真器才会真正生成弹丸。

`--preview` 需要：

```bash
pip install numpy opencv-python
```

## 5. Native Bridge 默认端口

| 方向 | 传输协议 | 默认地址 | 数据内容 |
|---|---|---|---|
| 自瞄 → 仿真器 | UDP | `127.0.0.1:39000` | 云台控制 JSON |
| 仿真器 → 自瞄 | UDP | `127.0.0.1:39001` | 位姿与统计 JSON |
| 仿真器 → 自瞄 | TCP | `127.0.0.1:39002` | 带帧头的 JPEG 图像流 |

详细字节格式和坐标系约定见：

[`docs/protocol.md`](docs/protocol.md)

## 6. 人工操作方式

默认按键：

| 输入 | 功能 |
|---|---|
| `W/A/S/D` | 控制射击机器人逻辑底盘平移 |
| 鼠标移动 | 手动控制云台 yaw / pitch |
| 按住鼠标右键 | 启用外部自瞄，外部自瞄接管云台 yaw / pitch |
| 按住鼠标左键 | 操作手允许开火 |
| `F1` | 锁定 / 释放鼠标，用于切换操作状态和 GUI 调参状态 |

### 6.1 手动控制云台

未按住右键时：

```text
mouse dx
   ↓
manual yaw

mouse dy
   ↓
manual pitch
```

此时外部自瞄程序即使持续发送 yaw/pitch，也不会抢走云台控制权。

### 6.2 启用自瞄

按住右键后：

```text
外部自瞄输出 yaw / pitch
          ↓
仿真器云台目标角
          ↓
云台执行机构
```

松开右键后立即恢复鼠标人工控制。

### 6.3 开火门控

人工模式下真正发射弹丸的条件为：

```text
RMB 按住
 AND
LMB 按住
 AND
外部自瞄命令未超时
 AND
auto-aim fire = true
 AND
发射冷却完成
        ↓
生成一颗 17 mm 弹丸
```

也就是说：

```text
左键只是“操作手允许开火”

真正的开火时机仍由
外部自瞄 Planner / Shooter 的 fire=true 决定
```

因此既不会出现“按左键就无脑连续射击”，也不会出现“自瞄 fire=true 后操作手没按左键仍然自动开火”的情况。

### 6.4 WASD 底盘运动

虽然射击端视觉上只显示：

```text
Camera
   │
Gimbal
   │
Barrel
   │
Muzzle
```

但内部结构为：

```text
ShooterRoot
     │
     └── Gimbal
          ├── Camera
          ├── Barrel
          └── Muzzle
```

`ShooterRoot` 是不可见的逻辑底盘，因此 WASD 会带动整个云台、枪管、相机和枪口一起移动。

默认移动方向相对于当前云台朝向：

```text
             W
             ↑

        A ← Shooter → D

             ↓
             S
```

如果希望改成世界坐标方向运动，可以在配置中设置：

```toml
[operator]
move_relative_to_gimbal = false
```

### 6.5 F1 操作模式

程序启动后默认锁定鼠标：

```text
鼠标移动
   ↓
控制云台
```

按一次 `F1`：

```text
释放鼠标
   ↓
可以点击右侧 egui 控制面板
```

再次按 `F1`：

```text
重新锁定鼠标
   ↓
恢复操作模式
```

在鼠标释放状态下，右键和左键不会触发自瞄与发射，避免点击 GUI 时误开火。

完整操作权仲裁和统计规则见：

[`docs/operator.md`](docs/operator.md)

## 7. 人工模式命中率与 DPS 统计

人工模式与自动 Benchmark 共用同一套弹丸和装甲碰撞链路：

```text
实际生成 Projectile
        ↓
Avian 物理仿真
        ↓
Armor Collision
        ↓
命中 / 伤害 / HP / DPS
```

每实际生成一颗弹丸：

```text
Shots += 1
```

只有弹丸真实碰撞到四块装甲板之一时才会：

```text
Hits += 1
Damage += damage_per_hit
HP -= damage_per_hit
```

实时统计包括：

- Shots
- Hits
- Hit Rate
- Total Damage
- Average DPS
- Rolling DPS
- Peak Rolling DPS
- Kill Time
- Target HP

命中率：

```text
Hit Rate = Hits / Shots × 100%
```

例如：

```text
实际发射 100 发
命中 73 发

Hit Rate = 73%
```

这里的 `Shots` 指**真正生成的弹丸数量**，而不是鼠标点击次数、`fire=true` 次数或 Planner 请求次数。

### Average DPS

点击 `Reset statistics` 后不会立即开始 DPS 计时。

计时从下一颗真正生成的弹丸开始：

```text
Reset statistics
      ↓
人工寻找目标 / 调整云台 / 移动底盘
      ↓
第一颗真实弹丸生成
      ↓
      t = 0
```

因此准备阶段不会稀释平均 DPS。

### Rolling DPS

默认窗口可配置，例如：

```toml
dps_window_s = 1.0
```

如果最近 1 秒命中 4 发，每发造成 10 damage：

```text
Rolling DPS = 40
```

这个指标特别适合观察小陀螺情况下的连续火力稳定性。

## 8. Benchmark 自动测试

自动 Benchmark 默认：

```toml
[operator]
benchmark_auto_hold_inputs = true
```

这相当于 Benchmark 自动模拟：

```text
RMB 一直按住
+
LMB 一直按住
```

但 Benchmark **不会替自瞄程序生成 `fire=true`**。

因此最终仍然是：

```text
Benchmark 自动启用自瞄和开火许可
              ↓
外部 Tracker / Predictor / Planner
              ↓
       fire=false → 不发射
       fire=true  → 发射
```

这意味着 Planner 的开火策略也是 Benchmark 的评测对象。

Benchmark 期间会禁用 WASD，并在每个 trial 开始前重新复位射击机器人位置，保证距离配置不会受到人工操作历史位置影响。

默认 sweep 位于 `config/default.toml`：

```toml
[benchmark]
distances_m = [3.0, 5.0, 7.0, 10.0]
rpms = [0.0, 30.0, 60.0, 120.0, 180.0]
translation_speeds_mps = [0.0, 1.0, 2.0, 3.0]
rounds_per_trial = 100
repeats_per_condition = 1
warmup_s = 1.0
case_timeout_s = 20.0
post_fire_grace_s = 1.2
```

即：

```text
4 个距离
×
5 个 RPM
×
4 个平移速度
=
80 个工况 / repeat
```

### Benchmark 状态机

```text
Reset
  ↓
Warmup
  │
  │  Detector / Tracker / Predictor 可以先收敛
  │  此阶段禁止计入测试开火
  ↓
Running
  │
  │  外部自瞄控制 yaw / pitch / fire
  │
  │  达到 N 颗真实弹丸
  │  或 case_timeout
  ↓
Drain
  │
  │  不再接受新弹丸
  │  等待已经在途的弹丸完成飞行
  ↓
写入 CSV
  ↓
下一工况
```

可以在 GUI 中启动，也可以直接无人值守运行：

```bash
cargo run --release -- --config config/benchmark_ci.toml
```

## 9. Benchmark 输出

每次 Benchmark 会生成：

```text
benchmark_results/run_<unix-time>/
├── benchmark_meta.csv
├── effective_config.toml
├── trials.csv
├── conditions.csv
└── rpm_degradation.csv
```

### trials.csv

记录每次独立试验，包括：

- 实际发射弹丸数
- 命中数
- 命中率
- 累计伤害
- Effective DPS
- Peak Rolling DPS
- 是否击杀
- Kill Time
- 是否超时
- 评测持续时间

### conditions.csv

对相同：

```text
distance × RPM × translation speed
```

的重复试验进行聚合。

### rpm_degradation.csv

按 RPM 进一步聚合，并在存在 `0 RPM` 基线时计算相对性能退化。

可以绘图：

```bash
pip install pandas matplotlib
python3 scripts/plot_benchmark.py benchmark_results/run_<unix-time>
```

用于生成：

- 命中率随 RPM 变化曲线
- DPS 随 RPM 变化曲线
- Kill Success 随 RPM 变化曲线
- 平均 TTK 随 RPM 变化曲线
- Hit Rate Degradation 曲线
- DPS Degradation 曲线

## 10. ROS 2 适配器

Rust 仿真核心**不直接链接 ROS 2**。

可选 ROS 2 桥接脚本：

```bash
python3 scripts/ros2_bridge.py
```

它负责把 Native Bridge 转换成标准 ROS 2 图像、CameraInfo、位姿和控制消息。

如果你的自瞄项目有自己的：

```text
GimbalCmd
AimInfo
共享内存结构
自定义 CAN / IPC 协议
```

只需要修改桥接层，不需要把这些协议写进仿真核心。

详细说明见：

[`docs/protocol.md`](docs/protocol.md)

## 11. 项目目录

```text
.
├── Cargo.toml
├── config/
│   ├── default.toml
│   └── benchmark_ci.toml
├── docs/
│   ├── architecture.md
│   ├── benchmark.md
│   ├── operator.md
│   └── protocol.md
├── scripts/
│   ├── native_client.py
│   ├── plot_benchmark.py
│   └── ros2_bridge.py
└── src/
    ├── main.rs
    ├── config.rs
    ├── components.rs
    ├── scene.rs
    ├── camera.rs
    ├── network.rs
    ├── protocol.rs
    ├── control.rs
    ├── projectile.rs
    ├── telemetry.rs
    ├── benchmark.rs
    └── ui.rs
```

## 12. 几何模型说明

项目内置的装甲板尺寸、前后半径和左右半径只是**仿真默认值**，并不表示它们与所有 RoboMaster 赛季、所有机器人或实际 CAD 完全一致。

如果需要和你的机器人一致，请在 TOML 配置中修改对应几何参数。

相机目前使用理想针孔模型，不模拟镜头畸变。

仿真器会根据相机参数得到并传出：

```text
fx
fy
cx
cy
```

相机和枪管之间的外参也显式写在配置中，因此可以复现相同实验条件。

## 13. 项目设计原则

核心目标是把三层解耦：

```text
仿真核心
Bevy + Avian 渲染与物理
        ↓
Native Bridge
稳定、与自瞄项目无关的 IPC
        ↓
Adapter
你的自瞄 / ROS2 / Talos / 自定义协议
```

这样即使以后你更换：

```text
Detector
Tracker
EKF / 因子图
Predictor
MPC
Planner
通信框架
```

仿真核心和 Benchmark 都可以继续复用。
