use std::f32::consts::TAU;

use bevy::{
    input::mouse::AccumulatedMouseMotion,
    prelude::*,
    window::{CursorGrabMode, CursorOptions},
};

use crate::{
    components::{
        AutoAimCommandState, EvaluationGate, Gimbal, GimbalState, OperatorState, ShooterRoot,
        TargetRoot, TargetRuntime,
    },
    config::{SimConfig, TargetPath},
    network::NetworkBridge,
};

/// Receive the latest external command regardless of the current operator mode.
/// RMB decides whether that cached command is actually allowed to steer the gimbal.
pub fn receive_external_commands(
    time: Res<Time>,
    config: Res<SimConfig>,
    bridge: Res<NetworkBridge>,
    mut external: ResMut<AutoAimCommandState>,
) {
    if let Some(cmd) = bridge.drain_latest_command() {
        external.yaw_deg = cmd
            .yaw_deg
            .clamp(-config.shooter.yaw_limit_deg, config.shooter.yaw_limit_deg);
        external.pitch_deg = cmd
            .pitch_deg
            .clamp(config.shooter.pitch_min_deg, config.shooter.pitch_max_deg);
        external.fire_advice = cmd.fire;
        external.last_rx_s = time.elapsed_secs_f64();
        external.ever_received = true;
    }
}

/// FPS/RoboMaster-style operator input.
///
/// Interactive mode:
/// - WASD: translate the shooter chassis/root.
/// - Mouse: manually steer gimbal when RMB is not held.
/// - Hold RMB: external auto-aim owns yaw/pitch.
/// - Hold LMB while RMB is held: permits firing, but only when external `fire=true`.
/// - F1: enter/leave robot control by toggling cursor capture.
///
/// Automated benchmark mode can emulate holding RMB+LMB, keeping CI unattended.
#[allow(clippy::too_many_arguments)]
pub fn operator_input(
    time: Res<Time>,
    config: Res<SimConfig>,
    keyboard: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    mouse_motion: Res<AccumulatedMouseMotion>,
    gate: Res<EvaluationGate>,
    external: Res<AutoAimCommandState>,
    mut operator: ResMut<OperatorState>,
    mut state: ResMut<GimbalState>,
    mut shooter: Single<&mut Transform, With<ShooterRoot>>,
    mut cursor: Single<&mut CursorOptions>,
) {
    let now = time.elapsed_secs_f64();

    if keyboard.just_pressed(KeyCode::F1) {
        operator.cursor_captured = !operator.cursor_captured;
    }
    cursor.visible = !operator.cursor_captured;
    cursor.grab_mode = if operator.cursor_captured {
        CursorGrabMode::Locked
    } else {
        CursorGrabMode::None
    };

    operator.command_fresh = external.ever_received
        && now - external.last_rx_s <= config.operator.command_timeout_s.max(0.01);

    let automated = gate.benchmark_active && config.operator.benchmark_auto_hold_inputs;
    operator.auto_aim_enabled = automated
        || (operator.cursor_captured && mouse.pressed(MouseButton::Right));
    operator.trigger_held = automated
        || (operator.cursor_captured && mouse.pressed(MouseButton::Left));

    // Manual chassis movement is deliberately disabled during automated benchmark cases so every
    // case starts from an identical, reproducible shooter pose.
    if !gate.benchmark_active && operator.cursor_captured {
        let mut axis = Vec2::ZERO;
        if keyboard.pressed(KeyCode::KeyW) {
            axis.y += 1.0;
        }
        if keyboard.pressed(KeyCode::KeyS) {
            axis.y -= 1.0;
        }
        if keyboard.pressed(KeyCode::KeyD) {
            axis.x += 1.0;
        }
        if keyboard.pressed(KeyCode::KeyA) {
            axis.x -= 1.0;
        }
        if axis != Vec2::ZERO {
            axis = axis.normalize();
            let local = Vec3::new(axis.x, 0.0, -axis.y);
            let world = if config.operator.move_relative_to_gimbal {
                Quat::from_rotation_y(state.yaw_deg.to_radians()) * local
            } else {
                local
            };
            shooter.translation +=
                world * config.operator.chassis_move_speed_mps.max(0.0) * time.delta_secs();
        }
    }

    if operator.auto_aim_enabled {
        // Only a fresh external command may own the gimbal. If the link goes stale, hold the last
        // target pose and force fire false.
        if operator.command_fresh {
            state.target_yaw_deg = external.yaw_deg;
            state.target_pitch_deg = external.pitch_deg;
        }
    } else if operator.cursor_captured {
        // Mouse manual aim. Bevy reports +X motion to the right and +Y downward.
        state.target_yaw_deg -=
            mouse_motion.delta.x * config.operator.mouse_sensitivity_yaw_deg.max(0.0);
        state.target_pitch_deg -=
            mouse_motion.delta.y * config.operator.mouse_sensitivity_pitch_deg.max(0.0);
    }

    state.target_yaw_deg = state.target_yaw_deg.clamp(
        -config.shooter.yaw_limit_deg,
        config.shooter.yaw_limit_deg,
    );
    state.target_pitch_deg = state
        .target_pitch_deg
        .clamp(config.shooter.pitch_min_deg, config.shooter.pitch_max_deg);

    // Triple gate in interactive mode:
    //   RMB(auto aim) && LMB(operator trigger) && external fire advice.
    // Benchmark mode emulates RMB+LMB but still requires external fire advice.
    state.fire_latched = operator.auto_aim_enabled
        && operator.trigger_held
        && operator.command_fresh
        && external.fire_advice;
}

pub fn update_gimbal_pose(
    time: Res<Time>,
    config: Res<SimConfig>,
    mut state: ResMut<GimbalState>,
    mut gimbal: Single<&mut Transform, With<Gimbal>>,
) {
    let dt = time.delta_secs();
    state.yaw_deg = move_towards(
        state.yaw_deg,
        state.target_yaw_deg,
        config.shooter.max_yaw_speed_dps * dt,
    );
    state.pitch_deg = move_towards(
        state.pitch_deg,
        state.target_pitch_deg,
        config.shooter.max_pitch_speed_dps * dt,
    );

    gimbal.rotation = Quat::from_rotation_y(state.yaw_deg.to_radians())
        * Quat::from_rotation_x(state.pitch_deg.to_radians());
}

pub fn update_target_motion(
    time: Res<Time>,
    target: Res<TargetRuntime>,
    mut transform: Single<&mut Transform, With<TargetRoot>>,
) {
    if target.freeze_when_dead && target.hp <= 0.0 {
        return;
    }

    let t = (time.elapsed_secs_f64() - target.phase_start_s).max(0.0) as f32;
    let yaw = target.rpm * TAU / 60.0 * t;
    let mut p = target.origin;
    let ax = target.half_extent_x_m.max(0.0);
    let az = target.half_extent_z_m.max(0.0);
    let v = target.translation_speed_mps.max(0.0);

    match target.path {
        TargetPath::Stationary => {}
        TargetPath::LineX => {
            let w = if ax > 1e-4 { v / ax } else { 0.0 };
            p.x += ax * (w * t).sin();
        }
        TargetPath::LineZ => {
            let w = if az > 1e-4 { v / az } else { 0.0 };
            p.z += az * (w * t).sin();
        }
        TargetPath::Ellipse => {
            let scale = ax.max(az).max(1e-4);
            let w = v / scale;
            p.x += ax * (w * t).sin();
            p.z += az * (w * t).cos();
        }
        TargetPath::FigureEight => {
            let scale = ax.max(az).max(1e-4);
            let w = v / scale;
            p.x += ax * (w * t).sin();
            p.z += az * 0.5 * (2.0 * w * t).sin();
        }
    }

    transform.translation = p;
    transform.rotation = Quat::from_rotation_y(yaw);
}

pub fn reset_target(
    now_s: f64,
    distance_m: f32,
    runtime: &mut TargetRuntime,
    transform: &mut Transform,
) {
    runtime.origin = Vec3::new(0.0, 0.0, -distance_m.max(0.1));
    runtime.phase_start_s = now_s;
    runtime.reset_hp();
    transform.translation = runtime.origin;
    transform.rotation = Quat::IDENTITY;
}

fn move_towards(current: f32, target: f32, max_delta: f32) -> f32 {
    let delta = target - current;
    if delta.abs() <= max_delta {
        target
    } else {
        current + delta.signum() * max_delta
    }
}
