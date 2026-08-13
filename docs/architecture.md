# Architecture

## Goal

The simulator is an external test instrument for an auto-aim system, not an auto-aim implementation itself.
It should make the external program believe it is operating a minimal robot sensor/actuator loop while retaining deterministic ground-truth collision scoring.

## Runtime data flow

```text
Bevy 3D world
  │
  ├─ target motion system ────────────────┐
  │                                       │
  ├─ off-screen aim camera ── JPEG ───────┼── Native Bridge ──► auto-aim
  │                                       │                    detector
  ├─ gimbal/camera/muzzle telemetry ──────┤                    tracker
  │                                       │                    predictor
  │                                       │                    planner
  │                                       │                       │
  │                                       │                       ▼
  └─ projectile + armor collision ◄───────┴── yaw/pitch/fire ◄────┘
                │
                ├─ HP
                ├─ hit rate
                ├─ rolling DPS
                ├─ kill time
                └─ benchmark CSV
```

## Modules

### `scene.rs`
Creates every visible/collidable object procedurally:

- ground/reference marks
- invisible/logical shooter chassis root
- gimbal
- barrel
- muzzle
- target root
- four armor plates

There is no target chassis mesh/collider.

### `camera.rs`
Creates one off-screen 3D camera as a child of the gimbal and one 2D preview camera.
The off-screen image is:

1. shown as the application background,
2. captured,
3. JPEG encoded,
4. pushed to the Native Bridge.

Therefore the external client sees the same view as the user sees behind the control panel.

### `network.rs` / `protocol.rs`
The project-independent IPC boundary.

- command RX: UDP JSON
- telemetry TX: UDP JSON
- camera TX: TCP framed JPEG

No ROS 2 or team-specific message type is linked into the core binary.

### `control.rs`
Owns operator/auto-aim arbitration and target motion. It:

- caches external yaw/pitch/fire commands independently of operator mode,
- maps WASD to shooter-root translation,
- maps raw mouse motion to manual gimbal targets,
- switches gimbal ownership to external auto-aim while RMB is held,
- builds the final fire gate from RMB + LMB + command freshness + external fire advice,
- rate-limits physical gimbal motion,
- updates target motion from absolute simulation time.

Automated benchmark mode can emulate RMB+LMB without bypassing external fire advice.

### `projectile.rs`
Spawns 17 mm projectiles from the muzzle, applies physics velocity, enables continuous collision detection, and registers armor collision events.

Only a projectile colliding with an armor entity increments hit statistics.

### `benchmark.rs`
Owns the sweep and evaluation state machine.
It deliberately counts **actual spawned projectiles**, not fire commands.
This preserves the behavior of the external planner/fire gate as part of the benchmark.

### `ui.rs`
Runtime operator controls and benchmark progress.

## Coordinate system

Bevy world convention used by this project:

- `+X`: screen/world right
- `+Y`: up
- shooter looks approximately toward `-Z` at zero yaw/pitch
- muzzle forward axis: local `-Z`
- camera is rigidly attached to the gimbal

The target starts at `(0, 0, -distance)`.

See `protocol.md` for command signs and quaternion ordering.

## Reproducibility

A benchmark run saves `effective_config.toml` and `benchmark_meta.csv` beside results.
Important physical conditions include:

- camera resolution/FPS/FOV
- camera-to-gimbal translation
- physics fixed Hz
- gravity
- projectile diameter/speed/mass/cooldown
- target geometry
- target path and translation space
- HP/damage model
- warmup/timeout/drain timing

Changing one of these should be treated as changing the benchmark environment.
