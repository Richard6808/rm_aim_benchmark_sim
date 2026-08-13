# Interactive operator mode

The standalone simulator supports an operator workflow intended to feel like a RoboMaster driver/vision test station while keeping the external auto-aim software in the loop.

## Controls

| Input | Action |
|---|---|
| `W` / `A` / `S` / `D` | translate the shooter chassis/root |
| mouse movement | manual gimbal yaw/pitch while auto-aim is not enabled |
| hold right mouse button | enable external auto-aim control of gimbal yaw/pitch |
| hold left mouse button | operator firing permission/trigger |
| `F1` | capture/release the cursor so the egui panel can be used |

By default WASD is relative to the current gimbal yaw, which makes `W` move in the horizontal viewing direction. Set `operator.move_relative_to_gimbal = false` to use fixed world axes instead.

When the cursor is released with `F1`, interactive RMB/LMB actuation is disabled so clicks on the control panel cannot accidentally enable auto-aim or fire.

## Auto-aim arbitration

The simulator always receives and caches the newest native `GimbalCommand`, but in interactive mode the external command is only allowed to steer the gimbal while RMB is held.

```text
RMB released
    mouse motion
        ↓
manual yaw / pitch

RMB held + fresh external command
        ↓
external yaw / pitch
```

A stale/disconnected command cannot fire. `operator.command_timeout_s` controls the freshness window.

## Fire gate

Interactive firing deliberately requires all of these conditions at the same time:

```text
RMB held
   AND
LMB held
   AND
external command is fresh
   AND
external fire == true
   AND
projectile cooldown is ready
        ↓
spawn one physical 17 mm projectile
```

So:

- the auto-aim planner decides whether it recommends firing;
- the human operator still decides whether firing is permitted by holding LMB;
- simply holding LMB never bypasses the auto-aim fire gate;
- simply receiving `fire=true` never fires unless auto-aim is enabled and the operator trigger is held.

## Manual score statistics

Interactive and automated modes share the same `ScoreBoard` and armor collision path. The simulator counts **physical spawned projectiles**, not mouse clicks or fire commands.

- `Shots`: actual projectiles spawned from the muzzle.
- `Hits`: unique spawned projectiles that collide with an armor plate.
- `Hit rate`: `Hits / Shots`.
- `Rolling DPS`: armor damage accumulated in the configured rolling window divided by that window length.
- `Peak rolling DPS`: maximum rolling DPS observed in the current score session.
- `Kill time`: starts at the first actual projectile of a manual score session and ends when target HP first reaches zero.

`Reset statistics` clears the score and HP. In manual mode the timer does not start until the next projectile is actually emitted.

## Automated Benchmark interaction

The Benchmark Runner must remain unattended. With the default:

```toml
[operator]
benchmark_auto_hold_inputs = true
```

Benchmark mode emulates holding RMB and LMB. It **does not** emulate `fire=true`; the external auto-aim still has to send fire advice for every shot. Therefore planner/fire-gate failures remain measurable.

WASD movement is disabled while an automated benchmark is active and the shooter pose is reset to the canonical origin for every trial, preserving distance definitions and repeatability.
