use std::time::{SystemTime, UNIX_EPOCH};

use bevy::prelude::*;

use crate::{
    components::{
        AimCamera, AutoAimCommandState, CameraIntrinsics, Gimbal, GimbalState, Muzzle, OperatorState,
        ScoreBoard, ShooterRoot, TargetRuntime,
    },
    config::SimConfig,
    network::NetworkBridge,
    protocol::{CameraInfoWire, PoseWire, TelemetryPacket},
};

#[derive(Resource)]
pub struct TelemetryTimer(pub Timer);

pub fn setup_telemetry(mut commands: Commands, config: Res<SimConfig>) {
    commands.insert_resource(TelemetryTimer(Timer::from_seconds(
        1.0 / config.network.telemetry_hz.max(1.0),
        TimerMode::Repeating,
    )));
}

#[allow(clippy::too_many_arguments)]
pub fn publish_telemetry(
    time: Res<Time>,
    config: Res<SimConfig>,
    mut timer: ResMut<TelemetryTimer>,
    bridge: Res<NetworkBridge>,
    state: Res<GimbalState>,
    operator: Res<OperatorState>,
    external: Res<AutoAimCommandState>,
    target: Res<TargetRuntime>,
    intrinsics: Res<CameraIntrinsics>,
    mut score: ResMut<ScoreBoard>,
    shooter: Single<&GlobalTransform, With<ShooterRoot>>,
    gimbal: Single<&GlobalTransform, With<Gimbal>>,
    muzzle: Single<&GlobalTransform, With<Muzzle>>,
    camera: Single<&GlobalTransform, With<AimCamera>>,
) {
    timer.0.tick(time.delta());
    if !timer.0.just_finished() {
        return;
    }

    let now = time.elapsed_secs_f64();
    let shooter = shooter.into_inner();
    let gimbal = gimbal.into_inner();
    let muzzle = muzzle.into_inner();
    let camera = camera.into_inner();
    let packet = TelemetryPacket {
        protocol: "rm-aim-sim/3",
        timestamp_ns: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or_default(),
        gimbal_yaw_deg: state.yaw_deg,
        gimbal_pitch_deg: state.pitch_deg,
        auto_aim_enabled: operator.auto_aim_enabled,
        operator_trigger_held: operator.trigger_held,
        external_fire_advice: external.fire_advice,
        external_command_fresh: operator.command_fresh,
        shooter_pose: pose(shooter),
        gimbal_pose: pose(gimbal),
        muzzle_pose: pose(muzzle),
        camera_pose: pose(camera),
        camera_info: CameraInfoWire {
            width: intrinsics.width,
            height: intrinsics.height,
            fx: intrinsics.fx,
            fy: intrinsics.fy,
            cx: intrinsics.cx,
            cy: intrinsics.cy,
        },
        target_hp: target.hp,
        target_max_hp: target.max_hp,
        target_angular_speed_rad_s: target.angular_speed_rad_s,
        target_translation_speed_mps: target.translation_speed_mps,
        shots: score.shots,
        hits: score.hits,
        hit_rate_pct: score.hit_rate_pct(),
        total_damage: score.total_damage,
        average_dps: score.average_dps(now),
        rolling_dps: score.rolling_dps(now, config.benchmark.dps_window_s),
    };

    if let Ok(json) = serde_json::to_string(&packet) {
        bridge.try_send_telemetry(json);
    }
}

fn pose(transform: &GlobalTransform) -> PoseWire {
    PoseWire {
        translation_m: transform.translation().to_array(),
        quaternion_xyzw: transform.rotation().to_array(),
    }
}
