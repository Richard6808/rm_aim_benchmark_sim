mod benchmark;
mod camera;
mod components;
mod config;
mod control;
mod network;
mod projectile;
mod protocol;
mod scene;
mod telemetry;
mod ui;

use anyhow::Result;
use avian3d::prelude::*;
use bevy::{prelude::*, window::WindowResolution};
use bevy_egui::{EguiPlugin, EguiPrimaryContextPass};
use clap::Parser;

use crate::{
    components::{AutoAimCommandState, GimbalState, OperatorState},
    config::SimConfig,
    network::NetworkBridge,
};

#[derive(Parser, Debug)]
#[command(version, about)]
struct Args {
    #[arg(short, long, default_value = "config/default.toml")]
    config: String,
}

fn main() -> Result<()> {
    let args = Args::parse();
    let config = SimConfig::load(&args.config)?;
    let network = NetworkBridge::start(&config.network)?;
    let cursor_captured = config.operator.cursor_grab_on_start;

    let mut app = App::new();
    app.add_plugins(
        DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: config.window.title.clone(),
                resolution: WindowResolution::new(config.window.width, config.window.height),
                ..default()
            }),
            ..default()
        }),
    )
    .add_plugins(PhysicsPlugins::default())
    .add_plugins(EguiPlugin::default())
    .insert_resource(Time::<Fixed>::from_hz(config.physics.fixed_hz.max(1.0) as f64))
    .insert_resource(Gravity(Vec3::NEG_Y * config.physics.gravity_mps2))
    .insert_resource(config)
    .insert_resource(network)
    .insert_resource(GimbalState::default())
    .insert_resource(AutoAimCommandState::default())
    .insert_resource(OperatorState {
        cursor_captured,
        ..default()
    })
    .add_systems(
        Startup,
        (
            camera::setup_preview_camera,
            scene::setup_scene,
            projectile::setup_projectiles,
            telemetry::setup_telemetry,
            benchmark::setup_benchmark,
        ),
    )
    .add_systems(PostStartup, camera::setup_capture_target)
    .add_systems(
        Update,
        (
            benchmark::benchmark_state_machine,
            control::receive_external_commands,
            control::operator_input,
            control::update_gimbal_pose,
            control::update_target_motion,
            projectile::despawn_expired_projectiles,
            camera::request_camera_frames,
        )
            .chain(),
    )
    .add_systems(
        PostUpdate,
        (
            projectile::launch_projectiles.after(TransformSystems::Propagate),
            telemetry::publish_telemetry.after(projectile::launch_projectiles),
        ),
    )
    .add_systems(EguiPrimaryContextPass, ui::control_panel);

    app.run();
    Ok(())
}
