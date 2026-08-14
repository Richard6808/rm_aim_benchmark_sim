use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::{Context, Result};
use bevy::prelude::*;
use serde::Serialize;

use crate::{
    components::{
        EvaluationGate, GimbalState, Projectile, ScoreBoard, ShooterRoot, TargetRoot, TargetRuntime,
    },
    config::SimConfig,
    control::reset_target,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BenchmarkPhase {
    Idle,
    Warmup,
    Running,
    Drain,
    Finished,
}

impl BenchmarkPhase {
    pub fn label(self) -> &'static str {
        match self {
            Self::Idle => "空闲",
            Self::Warmup => "预热中",
            Self::Running => "测试中",
            Self::Drain => "等待弹丸结算",
            Self::Finished => "已完成",
        }
    }

    pub fn active(self) -> bool {
        matches!(self, Self::Warmup | Self::Running | Self::Drain)
    }
}

#[derive(Debug, Clone)]
pub struct BenchmarkCase {
    pub distance_m: f32,
    pub rpm: f32,
    pub translation_speed_mps: f32,
    pub repeat: u32,
}

#[derive(Debug, Clone, Serialize)]
pub struct TrialResult {
    pub trial_id: u64,
    pub distance_m: f32,
    pub rpm: f32,
    pub translation_speed_mps: f32,
    pub repeat: u32,
    pub rounds_budget: u32,
    pub shots_fired: u64,
    pub hits: u64,
    pub hit_rate_pct: f32,
    pub total_damage: f32,
    pub effective_dps: f32,
    pub peak_rolling_dps: f32,
    pub killed: bool,
    pub kill_time_s: Option<f64>,
    pub timed_out: bool,
    pub evaluation_duration_s: f64,
}

#[derive(Resource)]
pub struct BenchmarkRunner {
    pub phase: BenchmarkPhase,
    pub case_index: usize,
    pub case_count: usize,
    pub current: Option<BenchmarkCase>,
    pub output_dir: Option<PathBuf>,
    pub results: Vec<TrialResult>,
    pub start_requested: bool,
    pub stop_requested: bool,
    pub last_error: Option<String>,
    pub completed: bool,
    cases: Vec<BenchmarkCase>,
    phase_started_s: f64,
    running_started_s: f64,
    current_trial_id: u64,
    current_timed_out: bool,
    autostart_checked: bool,
}

impl Default for BenchmarkRunner {
    fn default() -> Self {
        Self {
            phase: BenchmarkPhase::Idle,
            case_index: 0,
            case_count: 0,
            current: None,
            output_dir: None,
            results: Vec::new(),
            start_requested: false,
            stop_requested: false,
            last_error: None,
            completed: false,
            cases: Vec::new(),
            phase_started_s: 0.0,
            running_started_s: 0.0,
            current_trial_id: 0,
            current_timed_out: false,
            autostart_checked: false,
        }
    }
}

pub fn setup_benchmark(mut commands: Commands) {
    commands.insert_resource(BenchmarkRunner::default());
}

#[allow(clippy::too_many_arguments)]
pub fn benchmark_state_machine(
    mut commands: Commands,
    time: Res<Time>,
    config: Res<SimConfig>,
    mut runner: ResMut<BenchmarkRunner>,
    mut gate: ResMut<EvaluationGate>,
    mut target: ResMut<TargetRuntime>,
    mut score: ResMut<ScoreBoard>,
    mut gimbal: ResMut<GimbalState>,
    mut target_transform: Single<&mut Transform, (With<TargetRoot>, Without<ShooterRoot>)>,
    mut shooter_transform: Single<&mut Transform, (With<ShooterRoot>, Without<TargetRoot>)>,
    projectiles: Query<Entity, With<Projectile>>,
) {
    let now = time.elapsed_secs_f64();

    if !runner.autostart_checked {
        runner.autostart_checked = true;
        if config.benchmark.autostart {
            runner.start_requested = true;
        }
    }

    if runner.stop_requested {
        runner.stop_requested = false;
        stop_benchmark(&mut runner, &mut gate);
        return;
    }

    if runner.start_requested {
        runner.start_requested = false;
        match begin_benchmark(&config, now, &mut runner) {
            Ok(()) => {
                gate.benchmark_active = true;
                apply_current_case(
                    &config,
                    now,
                    &mut runner,
                    &mut gate,
                    &mut target,
                    &mut target_transform,
                    &mut shooter_transform,
                    &mut score,
                    &mut gimbal,
                    &projectiles,
                    &mut commands,
                );
            }
            Err(e) => runner.last_error = Some(format!("{e:#}")),
        }
        return;
    }

    match runner.phase {
        BenchmarkPhase::Idle | BenchmarkPhase::Finished => {}
        BenchmarkPhase::Warmup => {
            if now - runner.phase_started_s >= config.benchmark.warmup_s {
                // Score only the actual evaluation window; warmup lets the tracker converge first.
                score.reset(now);
                gate.accepting = true;
                gate.rounds_limit = config.benchmark.rounds_per_trial;
                runner.phase = BenchmarkPhase::Running;
                runner.phase_started_s = now;
                runner.running_started_s = now;
                runner.current_timed_out = false;
            }
        }
        BenchmarkPhase::Running => {
            let round_budget_reached = score.shots >= config.benchmark.rounds_per_trial as u64;
            let timed_out = now - runner.running_started_s >= config.benchmark.case_timeout_s;
            if round_budget_reached || timed_out {
                gate.accepting = false;
                runner.current_timed_out = timed_out && !round_budget_reached;
                runner.phase = BenchmarkPhase::Drain;
                runner.phase_started_s = now;
            }
        }
        BenchmarkPhase::Drain => {
            if now - runner.phase_started_s >= config.benchmark.post_fire_grace_s {
                finalize_current_case(
                    &config,
                    now,
                    &mut runner,
                    &mut gate,
                    &target,
                    &score,
                );
                if runner.case_index + 1 >= runner.case_count {
                    runner.phase = BenchmarkPhase::Finished;
                    runner.completed = true;
                    runner.current = None;
                    gate.benchmark_active = false;
                    gate.accepting = false;
                } else {
                    runner.case_index += 1;
                    apply_current_case(
                        &config,
                        now,
                        &mut runner,
                        &mut gate,
                        &mut target,
                        &mut target_transform,
                        &mut shooter_transform,
                        &mut score,
                        &mut gimbal,
                        &projectiles,
                        &mut commands,
                    );
                }
            }
        }
    }
}

fn begin_benchmark(config: &SimConfig, now: f64, runner: &mut BenchmarkRunner) -> Result<()> {
    let mut cases = Vec::new();
    for repeat in 1..=config.benchmark.repeats_per_condition.max(1) {
        for &distance_m in &config.benchmark.distances_m {
            for &rpm in &config.benchmark.rpms {
                for &translation_speed_mps in &config.benchmark.translation_speeds_mps {
                    cases.push(BenchmarkCase {
                        distance_m,
                        rpm,
                        translation_speed_mps,
                        repeat,
                    });
                }
            }
        }
    }
    anyhow::ensure!(!cases.is_empty(), "benchmark sweep contains no cases");

    let run_id = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or_default();
    let output_dir = Path::new(&config.benchmark.output_dir).join(format!("run_{run_id}"));
    fs::create_dir_all(&output_dir)
        .with_context(|| format!("create benchmark output {}", output_dir.display()))?;

    runner.phase = BenchmarkPhase::Warmup;
    runner.case_index = 0;
    runner.case_count = cases.len();
    runner.cases = cases;
    runner.results.clear();
    runner.output_dir = Some(output_dir.clone());
    runner.phase_started_s = now;
    runner.current_trial_id = 0;
    runner.completed = false;
    runner.last_error = None;
    runner.current_timed_out = false;

    write_meta(config, &output_dir)?;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn apply_current_case(
    config: &SimConfig,
    now: f64,
    runner: &mut BenchmarkRunner,
    gate: &mut EvaluationGate,
    target: &mut TargetRuntime,
    target_transform: &mut Transform,
    shooter_transform: &mut Transform,
    score: &mut ScoreBoard,
    gimbal: &mut GimbalState,
    projectiles: &Query<Entity, With<Projectile>>,
    commands: &mut Commands,
) {
    let case = runner.cases[runner.case_index].clone();
    runner.current = Some(case.clone());
    runner.current_trial_id += 1;
    runner.phase = BenchmarkPhase::Warmup;
    runner.phase_started_s = now;
    runner.current_timed_out = false;

    gate.benchmark_active = true;
    gate.accepting = false;
    gate.trial_id = runner.current_trial_id;
    gate.rounds_limit = config.benchmark.rounds_per_trial;

    target.rpm = case.rpm;
    target.translation_speed_mps = case.translation_speed_mps;
    target.path = config.target.path;
    target.half_extent_x_m = config.target.half_extent_x_m;
    target.half_extent_z_m = config.target.half_extent_z_m;
    target.max_hp = config.target.max_hp;
    target.damage_per_hit = config.target.damage_per_hit;
    target.freeze_when_dead = config.target.freeze_when_dead;
    // Benchmark distances are defined from a canonical shooter origin, not from wherever the
    // operator drove during the previous manual session.
    shooter_transform.translation = Vec3::ZERO;
    shooter_transform.rotation = Quat::IDENTITY;
    reset_target(now, case.distance_m, target, target_transform);
    score.reset(now);

    *gimbal = GimbalState::default();
    for entity in projectiles.iter() {
        commands.entity(entity).despawn();
    }
}

fn finalize_current_case(
    config: &SimConfig,
    now: f64,
    runner: &mut BenchmarkRunner,
    gate: &mut EvaluationGate,
    target: &TargetRuntime,
    score: &ScoreBoard,
) {
    let Some(case) = runner.current.clone() else {
        return;
    };
    let duration = (now - runner.running_started_s).max(1e-6);
    let result = TrialResult {
        trial_id: runner.current_trial_id,
        distance_m: case.distance_m,
        rpm: case.rpm,
        translation_speed_mps: case.translation_speed_mps,
        repeat: case.repeat,
        rounds_budget: config.benchmark.rounds_per_trial,
        shots_fired: score.shots,
        hits: score.hits,
        hit_rate_pct: score.hit_rate_pct(),
        total_damage: score.total_damage,
        effective_dps: score.total_damage / duration as f32,
        peak_rolling_dps: score.peak_rolling_dps,
        killed: target.hp <= 0.0,
        kill_time_s: score.kill_time_s,
        timed_out: runner.current_timed_out,
        evaluation_duration_s: duration,
    };
    runner.results.push(result);
    gate.accepting = false;

    if let Some(output_dir) = &runner.output_dir {
        if let Err(e) = write_outputs(config, output_dir, &runner.results) {
            runner.last_error = Some(format!("write benchmark CSV: {e:#}"));
        }
    }
}

fn stop_benchmark(runner: &mut BenchmarkRunner, gate: &mut EvaluationGate) {
    gate.benchmark_active = false;
    gate.accepting = false;
    runner.phase = BenchmarkPhase::Idle;
    runner.current = None;
}

fn write_meta(config: &SimConfig, dir: &Path) -> Result<()> {
    let mut w = csv::Writer::from_path(dir.join("benchmark_meta.csv"))?;
    w.write_record(["key", "value"])?;
    let rows = [
        ("project", "rm_aim_benchmark_sim".to_string()),
        ("physics_fixed_hz", config.physics.fixed_hz.to_string()),
        ("gravity_mps2", config.physics.gravity_mps2.to_string()),
        ("projectile_diameter_m", config.projectile.diameter_m.to_string()),
        ("projectile_speed_mps", config.projectile.speed_mps.to_string()),
        ("projectile_cooldown_s", config.projectile.cooldown_s.to_string()),
        ("camera_width", config.camera.width.to_string()),
        ("camera_height", config.camera.height.to_string()),
        ("camera_fps", config.camera.fps.to_string()),
        ("target_path", format!("{:?}", config.target.path)),
        ("target_half_extent_x_m", config.target.half_extent_x_m.to_string()),
        ("target_half_extent_z_m", config.target.half_extent_z_m.to_string()),
        ("target_max_hp", config.target.max_hp.to_string()),
        ("damage_per_hit", config.target.damage_per_hit.to_string()),
        ("rounds_per_trial", config.benchmark.rounds_per_trial.to_string()),
        ("repeats_per_condition", config.benchmark.repeats_per_condition.to_string()),
        ("warmup_s", config.benchmark.warmup_s.to_string()),
        ("case_timeout_s", config.benchmark.case_timeout_s.to_string()),
        ("post_fire_grace_s", config.benchmark.post_fire_grace_s.to_string()),
    ];
    for (key, value) in rows {
        w.write_record([key, value.as_str()])?;
    }
    w.flush()?;
    Ok(())
}

fn write_outputs(config: &SimConfig, dir: &Path, results: &[TrialResult]) -> Result<()> {
    write_trials(dir, results)?;
    write_conditions(dir, results)?;
    write_rpm_degradation(dir, results)?;
    // Snapshot the full effective configuration beside the CSVs for reproducibility.
    fs::write(dir.join("effective_config.toml"), toml::to_string_pretty(config)?)?;
    Ok(())
}

fn write_trials(dir: &Path, results: &[TrialResult]) -> Result<()> {
    let mut w = csv::Writer::from_path(dir.join("trials.csv"))?;
    for result in results {
        w.serialize(result)?;
    }
    w.flush()?;
    Ok(())
}

#[derive(Serialize)]
struct ConditionRow {
    distance_m: f32,
    rpm: f32,
    translation_speed_mps: f32,
    trials: usize,
    mean_hit_rate_pct: f32,
    mean_dps: f32,
    mean_peak_rolling_dps: f32,
    kill_success_rate_pct: f32,
    mean_kill_time_s: Option<f64>,
    timeout_rate_pct: f32,
}

fn write_conditions(dir: &Path, results: &[TrialResult]) -> Result<()> {
    let mut groups: BTreeMap<(u32, u32, u32), Vec<&TrialResult>> = BTreeMap::new();
    for r in results {
        groups
            .entry((
                r.distance_m.to_bits(),
                r.rpm.to_bits(),
                r.translation_speed_mps.to_bits(),
            ))
            .or_default()
            .push(r);
    }

    let mut w = csv::Writer::from_path(dir.join("conditions.csv"))?;
    for ((distance, rpm, speed), group) in groups {
        let n = group.len() as f32;
        let kill_times: Vec<f64> = group.iter().filter_map(|r| r.kill_time_s).collect();
        w.serialize(ConditionRow {
            distance_m: f32::from_bits(distance),
            rpm: f32::from_bits(rpm),
            translation_speed_mps: f32::from_bits(speed),
            trials: group.len(),
            mean_hit_rate_pct: group.iter().map(|r| r.hit_rate_pct).sum::<f32>() / n,
            mean_dps: group.iter().map(|r| r.effective_dps).sum::<f32>() / n,
            mean_peak_rolling_dps: group.iter().map(|r| r.peak_rolling_dps).sum::<f32>() / n,
            kill_success_rate_pct: group.iter().filter(|r| r.killed).count() as f32 * 100.0 / n,
            mean_kill_time_s: if kill_times.is_empty() {
                None
            } else {
                Some(kill_times.iter().sum::<f64>() / kill_times.len() as f64)
            },
            timeout_rate_pct: group.iter().filter(|r| r.timed_out).count() as f32 * 100.0 / n,
        })?;
    }
    w.flush()?;
    Ok(())
}

#[derive(Serialize, Clone)]
struct RpmRow {
    rpm: f32,
    trials: usize,
    hit_rate_pct: f32,
    hit_rate_degradation_vs_0rpm_pct: Option<f32>,
    mean_dps: f32,
    dps_degradation_vs_0rpm_pct: Option<f32>,
    kill_success_rate_pct: f32,
    mean_kill_time_s: Option<f64>,
    timeout_rate_pct: f32,
}

fn write_rpm_degradation(dir: &Path, results: &[TrialResult]) -> Result<()> {
    let mut groups: BTreeMap<u32, Vec<&TrialResult>> = BTreeMap::new();
    for r in results {
        groups.entry(r.rpm.to_bits()).or_default().push(r);
    }

    let mut rows = Vec::new();
    for (rpm_bits, group) in groups {
        let n = group.len() as f32;
        let kill_times: Vec<f64> = group.iter().filter_map(|r| r.kill_time_s).collect();
        rows.push(RpmRow {
            rpm: f32::from_bits(rpm_bits),
            trials: group.len(),
            hit_rate_pct: group.iter().map(|r| r.hit_rate_pct).sum::<f32>() / n,
            hit_rate_degradation_vs_0rpm_pct: None,
            mean_dps: group.iter().map(|r| r.effective_dps).sum::<f32>() / n,
            dps_degradation_vs_0rpm_pct: None,
            kill_success_rate_pct: group.iter().filter(|r| r.killed).count() as f32 * 100.0 / n,
            mean_kill_time_s: if kill_times.is_empty() {
                None
            } else {
                Some(kill_times.iter().sum::<f64>() / kill_times.len() as f64)
            },
            timeout_rate_pct: group.iter().filter(|r| r.timed_out).count() as f32 * 100.0 / n,
        });
    }
    rows.sort_by(|a, b| a.rpm.total_cmp(&b.rpm));

    if let Some(base) = rows.iter().find(|r| r.rpm.abs() < 1e-4).cloned() {
        for row in &mut rows {
            row.hit_rate_degradation_vs_0rpm_pct = degradation(base.hit_rate_pct, row.hit_rate_pct);
            row.dps_degradation_vs_0rpm_pct = degradation(base.mean_dps, row.mean_dps);
        }
    }

    let mut w = csv::Writer::from_path(dir.join("rpm_degradation.csv"))?;
    for row in rows {
        w.serialize(row)?;
    }
    w.flush()?;
    Ok(())
}

fn degradation(base: f32, value: f32) -> Option<f32> {
    if base.abs() < 1e-6 {
        None
    } else {
        Some((base - value) / base * 100.0)
    }
}
