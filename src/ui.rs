use std::{
    fs,
    sync::{Arc, atomic::Ordering},
};

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
    mut chinese_font_loaded: Local<bool>,
) -> Result {
    let ctx = contexts.ctx_mut()?;
    if !*chinese_font_loaded {
        install_chinese_font(ctx);
        *chinese_font_loaded = true;
    }

    let now = time.elapsed_secs_f64();
    let average_dps = score.average_dps(now);
    let rolling_dps = score.rolling_dps(now, config.benchmark.dps_window_s);
    egui::Window::new("aimsim_control")
        .title_bar(false)
        .fixed_pos(egui::pos2(8.0, 8.0))
        .fixed_size(egui::vec2(420.0, 700.0))
        .resizable(false)
        .vscroll(true)
        .show(ctx, |ui| {
            ui.heading("RoboMaster 瞄准模拟器");
            ui.small("当前相机画面与发送给自瞄客户端的图像完全一致。");
            ui.separator();

            ui.collapsing("靶标机器人", |ui| {
                egui::ComboBox::from_label("运动路径")
                    .selected_text(target.path.label())
                    .show_ui(ui, |ui| {
                        for path in TargetPath::ALL {
                            ui.selectable_value(&mut target.path, path, path.label());
                        }
                    });
                ui.add(
                    egui::Slider::new(&mut target.rpm, -300.0..=300.0)
                        .text("旋转速度 RPM"),
                );
                ui.add(
                    egui::Slider::new(&mut target.translation_speed_mps, 0.0..=5.0)
                        .text("平移速度 m/s"),
                );
                ui.add(
                    egui::Slider::new(&mut target.half_extent_x_m, 0.0..=5.0)
                        .text("X 轴半行程 m"),
                );
                ui.add(
                    egui::Slider::new(&mut target.half_extent_z_m, 0.0..=5.0)
                        .text("Z 轴半行程 m"),
                );
                ui.horizontal(|ui| {
                    ui.label("当前血量");
                    ui.add(egui::DragValue::new(&mut target.hp).range(0.0..=20_000.0));
                    ui.label(format!("/ {:.0}", target.max_hp));
                });
                ui.add(
                    egui::DragValue::new(&mut target.max_hp)
                        .range(1.0..=20_000.0)
                        .prefix("最大血量 "),
                );
                ui.add(
                    egui::DragValue::new(&mut target.damage_per_hit)
                        .range(0.1..=1000.0)
                        .prefix("单发伤害 "),
                );
                ui.checkbox(&mut target.freeze_when_dead, "血量为 0 时停止运动");
                ui.horizontal(|ui| {
                    if ui.button("重置血量").clicked() {
                        target.reset_hp();
                    }
                    if ui.button("重置靶标位置").clicked() {
                        let distance = -target.origin.z;
                        reset_target(now, distance, &mut target, &mut target_transform);
                    }
                });
            });

            ui.collapsing("操作手 / 发射机构 / 通信", |ui| {
                ui.strong("交互操作");
                ui.label("W/A/S/D：移动发射机器人底盘");
                ui.label("鼠标：手动控制云台瞄准");
                ui.label("按住鼠标右键：启用外部自瞄");
                ui.label("手动模式按住左键：直接发射");
                ui.label("自瞄模式：右键 + 左键 + 最新自瞄指令允许发射");
                ui.label("F1：进入或退出机器人控制模式");
                ui.separator();

                ui.label(format!(
                    "瞄准模式：{} | 扳机：{} | 指令：{}",
                    if operator.auto_aim_enabled {
                        "自动瞄准"
                    } else {
                        "手动瞄准"
                    },
                    if operator.trigger_held {
                        "已按下"
                    } else {
                        "已松开"
                    },
                    if operator.command_fresh {
                        "有效"
                    } else {
                        "过期或未收到"
                    }
                ));
                ui.label(format!(
                    "控制模式：{}",
                    if operator.cursor_captured {
                        "机器人控制"
                    } else {
                        "界面操作（按 F1 控制机器人）"
                    }
                ));
                ui.label(format!(
                    "自瞄建议：偏航 {:+.2}°，俯仰 {:+.2}°，允许发射 {}",
                    external.yaw_deg,
                    external.pitch_deg,
                    if external.fire_advice { "是" } else { "否" }
                ));
                ui.label(format!(
                    "云台实际值：偏航 {:+.2}°，俯仰 {:+.2}°，发射门控 {}",
                    gimbal.yaw_deg,
                    gimbal.pitch_deg,
                    if gimbal.fire_latched { "开启" } else { "关闭" }
                ));
                ui.label(format!(
                    "发射机器人位置：X {:+.2} m，Z {:+.2} m",
                    shooter_transform.translation.x, shooter_transform.translation.z
                ));

                ui.add(
                    egui::Slider::new(&mut config.operator.chassis_move_speed_mps, 0.0..=8.0)
                        .text("WASD 移动速度 m/s"),
                );
                ui.add(
                    egui::Slider::new(&mut config.operator.mouse_sensitivity_yaw_deg, 0.01..=0.5)
                        .text("鼠标偏航灵敏度 °/单位"),
                );
                ui.add(
                    egui::Slider::new(&mut config.operator.mouse_sensitivity_pitch_deg, 0.01..=0.5)
                        .text("鼠标俯仰灵敏度 °/单位"),
                );
                ui.checkbox(
                    &mut config.operator.move_relative_to_gimbal,
                    "WASD 移动方向跟随云台偏航",
                );

                ui.horizontal(|ui| {
                    if ui.button("云台回中").clicked() {
                        gimbal.target_yaw_deg = 0.0;
                        gimbal.target_pitch_deg = 0.0;
                        gimbal.fire_latched = false;
                    }
                    if ui.button("重置发射机器人位置").clicked() {
                        shooter_transform.translation = Vec3::ZERO;
                        shooter_transform.rotation = Quat::IDENTITY;
                    }
                });

                ui.separator();
                ui.label(format!("控制指令 UDP：{}", config.network.command_bind));
                ui.label(format!("遥测数据 UDP：{}", config.network.telemetry_target));
                ui.label(format!("相机图像 TCP：{}", config.network.camera_bind));
                ui.label(format!(
                    "已接收指令数：{}",
                    bridge.stats.commands_received.load(Ordering::Relaxed)
                ));
                ui.label(format!(
                    "相机客户端数：{}",
                    bridge.stats.camera_clients.load(Ordering::Relaxed)
                ));
            });

            ui.collapsing("实时成绩", |ui| {
                ui.label(format!("发射数：{}", score.shots));
                ui.label(format!("命中数：{}", score.hits));
                ui.label(format!("命中率：{:.2}%", score.hit_rate_pct()));
                ui.label(format!("总伤害：{:.1}", score.total_damage));
                ui.label(format!("平均 DPS：{:.2}", average_dps));
                ui.label(format!("滑动窗口 DPS：{:.2}", rolling_dps));
                ui.label(match score.kill_time_s {
                    Some(v) => format!("击杀用时：{:.3} 秒", v),
                    None => "击杀用时：--".to_string(),
                });
                if ui.button("重置统计数据").clicked() {
                    score.reset_manual();
                    target.reset_hp();
                }
            });

            ui.separator();
            ui.heading("自动评测");
            let total_conditions = config.benchmark.distances_m.len()
                * config.benchmark.rpms.len()
                * config.benchmark.translation_speeds_mps.len()
                * config.benchmark.repeats_per_condition as usize;
            ui.label(format!(
                "参数扫描：{} 个距离 × {} 个转速 × {} 个平移速度 × {} 次重复 = {} 组测试",
                config.benchmark.distances_m.len(),
                config.benchmark.rpms.len(),
                config.benchmark.translation_speeds_mps.len(),
                config.benchmark.repeats_per_condition,
                total_conditions
            ));
            ui.add(
                egui::DragValue::new(&mut config.benchmark.rounds_per_trial)
                    .range(1..=10_000)
                    .prefix("每组发射数 "),
            );
            ui.add(
                egui::DragValue::new(&mut config.benchmark.repeats_per_condition)
                    .range(1..=100)
                    .prefix("重复次数 "),
            );
            ui.add(
                egui::DragValue::new(&mut config.benchmark.warmup_s)
                    .range(0.0..=30.0)
                    .speed(0.1)
                    .prefix("预热时间（秒） "),
            );
            ui.add(
                egui::DragValue::new(&mut config.benchmark.case_timeout_s)
                    .range(1.0..=300.0)
                    .speed(0.5)
                    .prefix("超时时间（秒） "),
            );

            ui.label(format!("状态：{}", benchmark.phase.label()));
            if benchmark.case_count > 0 {
                let done = benchmark.case_index.min(benchmark.case_count - 1);
                let progress = if benchmark.phase == crate::benchmark::BenchmarkPhase::Finished {
                    1.0
                } else {
                    done as f32 / benchmark.case_count as f32
                };
                ui.add(egui::ProgressBar::new(progress).show_percentage());
                ui.label(format!(
                    "测试进度：{}/{}",
                    benchmark.case_index + 1,
                    benchmark.case_count
                ));
            }
            if let Some(case) = &benchmark.current {
                ui.label(format!(
                    "当前参数：{:.1} m | {:.0} RPM | {:.1} m/s | 第 {} 次",
                    case.distance_m, case.rpm, case.translation_speed_mps, case.repeat
                ));
            }
            if let Some(path) = &benchmark.output_dir {
                ui.small(format!("输出目录：{}", path.display()));
            }
            if let Some(error) = &benchmark.last_error {
                ui.colored_label(egui::Color32::RED, error);
            }

            ui.horizontal(|ui| {
                let can_start = !benchmark.phase.active();
                if ui
                    .add_enabled(can_start, egui::Button::new("开始评测"))
                    .clicked()
                {
                    benchmark.start_requested = true;
                }
                if ui
                    .add_enabled(benchmark.phase.active(), egui::Button::new("停止"))
                    .clicked()
                {
                    benchmark.stop_requested = true;
                }
            });

            ui.separator();
            ui.small("手动模式与自动评测使用相同的实体弹丸和装甲命中事件。按 F1 可进入或退出机器人控制模式。");
        });

    Ok(())
}

fn install_chinese_font(ctx: &egui::Context) {
    let candidates = [
        r"C:\Windows\Fonts\msyh.ttc",
        r"C:\Windows\Fonts\simhei.ttf",
        "/System/Library/Fonts/PingFang.ttc",
        "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc",
    ];
    let Some(font_bytes) = candidates.iter().find_map(|path| fs::read(path).ok()) else {
        warn!("未找到可用的中文字体，控制面板中的中文可能无法正常显示");
        return;
    };

    let mut fonts = egui::FontDefinitions::default();
    let font_name = "aimsim_chinese".to_owned();
    fonts.font_data.insert(
        font_name.clone(),
        Arc::new(egui::FontData::from_owned(font_bytes)),
    );
    for family in [egui::FontFamily::Proportional, egui::FontFamily::Monospace] {
        fonts
            .families
            .entry(family)
            .or_default()
            .insert(0, font_name.clone());
    }
    ctx.set_fonts(fonts);
}
