# Native Bridge Protocol

Protocol version: `rm-aim-sim/2`

The Native Bridge is intentionally simple and language-independent.

## 1. Gimbal command: UDP JSON

Simulator listens at the configured `network.command_bind`, default:

```text
127.0.0.1:39000
```

Packet:

```json
{
  "yaw_deg": 12.5,
  "pitch_deg": -4.2,
  "fire": true
}
```

Semantics:

- values are **absolute gimbal setpoints**, not delta commands;
- angles are degrees;
- the simulator applies configured yaw/pitch limits and maximum axis slew speeds;
- `fire=true` is external **fire advice**, not unconditional firing; interactive mode still requires RMB + LMB;
- command freshness is controlled by `operator.command_timeout_s` (default `0.35 s`); a stale command can never fire;
- while RMB is released, mouse motion owns the gimbal and cached external yaw/pitch are not applied.

Current geometric sign convention:

- zero pose looks along world `-Z`;
- positive pitch rotates the muzzle upward;
- positive yaw uses Bevy's positive rotation about `+Y` (from the initial `-Z` direction this turns toward `-X`).

If your lower computer uses the opposite yaw sign, negate yaw in your adapter rather than changing the simulation world convention.

## 2. Telemetry: UDP JSON

Simulator sends to `network.telemetry_target`, default:

```text
127.0.0.1:39001
```

Example shape:

```json
{
  "protocol": "rm-aim-sim/2",
  "timestamp_ns": 1786610000000000000,
  "gimbal_yaw_deg": 12.1,
  "gimbal_pitch_deg": -4.0,
  "auto_aim_enabled": true,
  "operator_trigger_held": true,
  "external_fire_advice": true,
  "external_command_fresh": true,
  "shooter_pose": {
    "translation_m": [0.0, 0.0, 0.0],
    "quaternion_xyzw": [0.0, 0.0, 0.0, 1.0]
  },
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
  "average_dps": 26.0,
  "rolling_dps": 40.0
}
```

Notes:

- `timestamp_ns` is Unix/system time in nanoseconds;
- translations are metres in world coordinates;
- quaternion order is `[x, y, z, w]`;
- camera is a perfect pinhole camera in the core model;
- distortion coefficients are all zero unless a future camera model explicitly adds distortion.

## 3. Camera stream: TCP framed JPEG

Simulator listens at `network.camera_bind`, default:

```text
127.0.0.1:39002
```

A client connects and repeatedly reads:

```text
40-byte big-endian header
JPEG payload
40-byte big-endian header
JPEG payload
...
```

Header layout:

| Offset | Bytes | Type | Meaning |
|---:|---:|---|---|
| 0 | 8 | bytes | ASCII magic `AIMSIM01` |
| 8 | 8 | `u64` | frame ID |
| 16 | 8 | `u64` | capture timestamp ns |
| 24 | 4 | `u32` | width |
| 28 | 4 | `u32` | height |
| 32 | 4 | `u32` | JPEG payload length |
| 36 | 4 | `u32` | reserved, currently zero |

Python format string:

```python
struct.Struct("!8sQQIIII")
```

The following `jpeg_len` bytes are a complete JPEG image.

## 4. Timing

Camera frames and telemetry both carry system-clock nanosecond timestamps so an adapter can place them into one time domain.
For precise latency research, measure additional timestamps inside your detector/tracker/planner and do not replace simulator capture timestamps with receive time.

## 5. Adapter policy

Keep team-specific protocol conversion outside the Rust simulation core.
Examples:

```text
Native Bridge <-> ROS 2 adapter <-> your ROS 2 auto-aim
Native Bridge <-> Talos adapter  <-> shared-memory auto-aim
Native Bridge <-> C++ client     <-> existing non-ROS pipeline
```

This lets the same benchmark compare different auto-aim codebases without changing simulation physics or scoring.

## Operator gating semantics

The command packet remains intentionally simple:

```json
{"yaw_deg": 12.5, "pitch_deg": -4.2, "fire": true}
```

In interactive mode this packet is **advice/control input**, not unconditional actuation:

- yaw/pitch are applied only while the operator holds RMB and the packet is fresh;
- `fire=true` can spawn a projectile only while RMB and LMB are both held;
- if the command age exceeds `operator.command_timeout_s`, the fire gate is forced false.

Automated Benchmark can emulate RMB+LMB, but never emulates external `fire=true`.

Telemetry additionally exposes:

- `auto_aim_enabled`
- `operator_trigger_held`
- `external_fire_advice`
- `external_command_fresh`
- `shooter_pose`

These fields let a client/logging tool distinguish planner advice from the final operator-gated actuation state.
