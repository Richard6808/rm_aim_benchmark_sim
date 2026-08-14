use std::sync::atomic::Ordering;

use bevy::prelude::*;
use bevy_egui::{EguiContexts, egui};

use crate::{
    benchmark::BenchmarkRunner,
    components::{
        AutoAimCommandState, GimbalState, OperatorState, ScoreBoard, ShooterRoot, TargetRoot,
        TargetRuntime,
    },
    config::{SimConfig, TargetPath},
    control::reset_target,
    network::NetworkBridge,
};

#[allow(clippy::too_many_arguments)]
pub fn control_panel(
    mut contexts: EguiContexts,
    time: Res<Time>,
    mut config: ResMut<SimConfig>,
    mut target: ResMut<TargetRuntime>,
    mut target_transform: Single<&mut Transform, (With<TargetRoot>, Without<ShooterRoot>)>,
    mut gimbal: ResMut<GimbalState>,
    operator: Res<OperatorState>,
    external: Res<AutoAimCommandState>,
    mut shooter_transform: Single<&mut Transform, (With<ShooterRoot>, Without<TargetRoot>)>,
    mut score: ResMut<ScoreBoard>,
    bridge: Res<NetworkBridge>,
    mut benchmark: ResMut<BenchmarkRunner>,
) -> Result {
    let ctx = contexts.ctx_mut()?;
    let now = time.elapsed_secs_f64();
    let average_dps = score.average_dps(now);
    let rolling_dps = score.rolling_dps(now, config.benchmark.dps_window_s);
    let mut viewport_ui = egui::Ui::new(
        ctx.clone(),
        "aimsim_viewport".into(),
        egui::UiBuilder::new()
            .layer_id(egui::LayerId::background())
            .max_rect(ctx.viewport_rect()),
    );

    egui::Panel::right("aimsim_control")
        .default_size(320.0)
        .resizable(true)
        .show(&mut viewport_ui, |ui| {
            ui.heading("RoboMaster AimSim");
            ui.small("Camera view is the actual image exported to the auto-aim client.");
            ui.separator();

            ui.collapsing("Target robot", |ui| {
                egui::ComboBox::from_label("Path")
                    .selected_text(target.path.label())
                    .show_ui(ui, |ui| {
                        for path in TargetPath::ALL {
                            ui.selectable_value(&mut target.path, path, path.label());
                        }
                    });
                ui.add(egui::Slider::new(&mut target.rpm, -300.0..=300.0).text("RPM"));
                ui.add(
                    egui::Slider::new(&mut target.translation_speed_mps, 0.0..=5.0)
                        .text("translation m/s"),
                );
                ui.add(
                    egui::Slider::new(&mut target.half_extent_x_m, 0.0..=5.0)
                        .text("X half extent m"),
                );
                ui.add(
                    egui::Slider::new(&mut target.half_extent_z_m, 0.0..=5.0)
                        .text("Z half extent m"),
                );
                ui.horizontal(|ui| {
                    ui.label("HP");
                    ui.add(egui::DragValue::new(&mut target.hp).range(0.0..=20_000.0));
                    ui.label(format!("/ {:.0}", target.max_hp));
                });
                ui.add(
                    egui::DragValue::new(&mut target.max_hp)
                        .range(1.0..=20_000.0)
                        .prefix("max HP "),
                );
                ui.add(
                    egui::DragValue::new(&mut target.damage_per_hit)
                        .range(0.1..=1000.0)
                        .prefix("damage/hit "),
                );
                ui.checkbox(&mut target.freeze_when_dead, "Freeze when HP = 0");
                ui.horizontal(|ui| {
                    if ui.button("Reset HP").clicked() {
                        target.reset_hp();
                    }
                    if ui.button("Reset pose").clicked() {
                        let distance = -target.origin.z;
                        reset_target(now, distance, &mut target, &mut target_transform);
                    }
                });
            });

            ui.collapsing("Operator / shooter / link", |ui| {
                ui.strong("Interactive controls");
                ui.label("W/A/S/D  move shooter chassis");
                ui.label("Mouse    manual gimbal aim");
                ui.label("Hold RMB enable external auto-aim");
                ui.label("Hold LMB operator trigger");
                ui.label("Fire = RMB + LMB + fresh auto-aim fire=true");
                ui.label("F1       enter/leave robot control");
                ui.separator();

                ui.label(format!(
                    "Mode: {} | Trigger: {} | Command: {}",
                    if operator.auto_aim_enabled { "AUTO-AIM" } else { "MANUAL" },
                    if operator.trigger_held { "HELD" } else { "released" },
                    if operator.command_fresh { "fresh" } else { "STALE/NONE" }
                ));
                ui.label(format!("Mode: {}", if operator.cursor_captured { "ROBOT CONTROL" } else { "GUI (press F1 to control)" }));
                ui.label(format!(
                    "Auto-aim advice: yaw {:+.2}°, pitch {:+.2}°, fire {}",
                    external.yaw_deg, external.pitch_deg, external.fire_advice
                ));
                ui.label(format!(
                    "Applied gimbal: yaw {:+.2}°, pitch {:+.2}°, projectile gate {}",
                    gimbal.yaw_deg, gimbal.pitch_deg, gimbal.fire_latched
                ));
                ui.label(format!(
                    "Shooter XY/Z: x {:+.2} m, z {:+.2} m",
                    shooter_transform.translation.x, shooter_transform.translation.z
                ));

                ui.add(
                    egui::Slider::new(&mut config.operator.chassis_move_speed_mps, 0.0..=8.0)
                        .text("WASD speed m/s"),
                );
                ui.add(
                    egui::Slider::new(&mut config.operator.mouse_sensitivity_yaw_deg, 0.01..=0.5)
                        .text("mouse yaw deg/unit"),
                );
                ui.add(
                    egui::Slider::new(&mut config.operator.mouse_sensitivity_pitch_deg, 0.01..=0.5)
                        .text("mouse pitch deg/unit"),
                );
                ui.checkbox(
                    &mut config.operator.move_relative_to_gimbal,
                    "WASD relative to gimbal yaw",
                );

                ui.horizontal(|ui| {
                    if ui.button("Center gimbal").clicked() {
                        gimbal.target_yaw_deg = 0.0;
                        gimbal.target_pitch_deg = 0.0;
                        gimbal.fire_latched = false;
                    }
                    if ui.button("Reset shooter pose").clicked() {
                        shooter_transform.translation = Vec3::ZERO;
                        shooter_transform.rotation = Quat::IDENTITY;
                    }
                });

                ui.separator();
                ui.label(format!("Command UDP: {}", config.network.command_bind));
                ui.label(format!("Telemetry UDP: {}", config.network.telemetry_target));
                ui.label(format!("Camera TCP: {}", config.network.camera_bind));
                ui.label(format!(
                    "RX commands: {}",
                    bridge.stats.commands_received.load(Ordering::Relaxed)
                ));
                ui.label(format!(
                    "Camera clients: {}",
                    bridge.stats.camera_clients.load(Ordering::Relaxed)
                ));
            });

            ui.collapsing("Live score", |ui| {
                ui.label(format!("Shots: {}", score.shots));
                ui.label(format!("Hits: {}", score.hits));
                ui.label(format!("Hit rate: {:.2}%", score.hit_rate_pct()));
                ui.label(format!("Damage: {:.1}", score.total_damage));
                ui.label(format!("Average DPS: {:.2}", average_dps));
                ui.label(format!("Rolling DPS: {:.2}", rolling_dps));
                ui.label(match score.kill_time_s {
                    Some(v) => format!("Kill time: {:.3} s", v),
                    None => "Kill time: --".to_string(),
                });
                if ui.button("Reset statistics").clicked() {
                    score.reset_manual();
                    target.reset_hp();
                }
            });

            ui.separator();
            ui.heading("Benchmark");
            let total_conditions = config.benchmark.distances_m.len()
                * config.benchmark.rpms.len()
                * config.benchmark.translation_speeds_mps.len()
                * config.benchmark.repeats_per_condition as usize;
            ui.label(format!(
                "Sweep: {} distances × {} RPM × {} speeds × {} repeats = {} trials",
                config.benchmark.distances_m.len(),
                config.benchmark.rpms.len(),
                config.benchmark.translation_speeds_mps.len(),
                config.benchmark.repeats_per_condition,
                total_conditions
            ));
            ui.add(
                egui::DragValue::new(&mut config.benchmark.rounds_per_trial)
                    .range(1..=10_000)
                    .prefix("shots/trial "),
            );
            ui.add(
                egui::DragValue::new(&mut config.benchmark.repeats_per_condition)
                    .range(1..=100)
                    .prefix("repeats "),
            );
            ui.add(
                egui::DragValue::new(&mut config.benchmark.warmup_s)
                    .range(0.0..=30.0)
                    .speed(0.1)
                    .prefix("warmup s "),
            );
            ui.add(
                egui::DragValue::new(&mut config.benchmark.case_timeout_s)
                    .range(1.0..=300.0)
                    .speed(0.5)
                    .prefix("timeout s "),
            );

            ui.label(format!("State: {}", benchmark.phase.label()));
            if benchmark.case_count > 0 {
                let done = benchmark.case_index.min(benchmark.case_count - 1);
                let progress = if benchmark.phase == crate::benchmark::BenchmarkPhase::Finished {
                    1.0
                } else {
                    done as f32 / benchmark.case_count as f32
                };
                ui.add(egui::ProgressBar::new(progress).show_percentage());
                ui.label(format!(
                    "Trial {}/{}",
                    benchmark.case_index + 1,
                    benchmark.case_count
                ));
            }
            if let Some(case) = &benchmark.current {
                ui.label(format!(
                    "Current: {:.1} m | {:.0} RPM | {:.1} m/s | repeat {}",
                    case.distance_m, case.rpm, case.translation_speed_mps, case.repeat
                ));
            }
            if let Some(path) = &benchmark.output_dir {
                ui.small(format!("Output: {}", path.display()));
            }
            if let Some(error) = &benchmark.last_error {
                ui.colored_label(egui::Color32::RED, error);
            }

            ui.horizontal(|ui| {
                let can_start = !benchmark.phase.active();
                if ui
                    .add_enabled(can_start, egui::Button::new("Start benchmark"))
                    .clicked()
                {
                    benchmark.start_requested = true;
                }
                if ui
                    .add_enabled(benchmark.phase.active(), egui::Button::new("Stop"))
                    .clicked()
                {
                    benchmark.stop_requested = true;
                }
            });

            ui.separator();
            ui.small("Manual session score uses the same physical projectiles and armor-hit events as Benchmark. Press F1 to enter or leave robot control.");
        });

    Ok(())
}
