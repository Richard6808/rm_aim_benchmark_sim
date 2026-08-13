use std::collections::{HashSet, VecDeque};

use bevy::prelude::*;

#[derive(Component)]
pub struct ShooterRoot;

#[derive(Component)]
pub struct Gimbal;

#[derive(Component)]
pub struct Barrel;

#[derive(Component)]
pub struct Muzzle;

#[derive(Component)]
pub struct AimCamera;

#[derive(Component)]
pub struct TargetRoot;

#[derive(Component, Debug, Clone, Copy)]
pub struct ArmorPlate {
    pub index: u8,
}

#[derive(Component)]
pub struct Projectile {
    pub shot_id: u64,
    pub counted_hit: bool,
}

#[derive(Component)]
pub struct ProjectileLifetime(pub Timer);

#[derive(Component)]
pub struct AcceptedBenchmarkProjectile {
    pub trial_id: u64,
}

#[derive(Resource, Debug, Clone, Copy)]
pub struct GimbalState {
    pub yaw_deg: f32,
    pub pitch_deg: f32,
    pub target_yaw_deg: f32,
    pub target_pitch_deg: f32,
    /// Final permission consumed by the projectile launcher after all operator/benchmark gates.
    pub fire_latched: bool,
}

impl Default for GimbalState {
    fn default() -> Self {
        Self {
            yaw_deg: 0.0,
            pitch_deg: 0.0,
            target_yaw_deg: 0.0,
            target_pitch_deg: 0.0,
            fire_latched: false,
        }
    }
}

#[derive(Resource, Debug, Clone, Copy)]
pub struct AutoAimCommandState {
    pub yaw_deg: f32,
    pub pitch_deg: f32,
    pub fire_advice: bool,
    pub last_rx_s: f64,
    pub ever_received: bool,
}

impl Default for AutoAimCommandState {
    fn default() -> Self {
        Self {
            yaw_deg: 0.0,
            pitch_deg: 0.0,
            fire_advice: false,
            last_rx_s: -1.0e9,
            ever_received: false,
        }
    }
}

#[derive(Resource, Debug, Clone, Copy)]
pub struct OperatorState {
    /// Physical right mouse button in interactive mode; forced true by automated benchmark mode.
    pub auto_aim_enabled: bool,
    /// Physical left mouse button in interactive mode; forced true by automated benchmark mode.
    pub trigger_held: bool,
    pub command_fresh: bool,
    pub cursor_captured: bool,
}

impl Default for OperatorState {
    fn default() -> Self {
        Self {
            auto_aim_enabled: false,
            trigger_held: false,
            command_fresh: false,
            cursor_captured: true,
        }
    }
}

#[derive(Resource, Debug, Clone)]
pub struct TargetRuntime {
    pub origin: Vec3,
    pub phase_start_s: f64,
    pub rpm: f32,
    pub translation_speed_mps: f32,
    pub half_extent_x_m: f32,
    pub half_extent_z_m: f32,
    pub path: crate::config::TargetPath,
    pub max_hp: f32,
    pub hp: f32,
    pub damage_per_hit: f32,
    pub freeze_when_dead: bool,
}

impl TargetRuntime {
    pub fn reset_hp(&mut self) {
        self.hp = self.max_hp;
    }
}

#[derive(Resource, Debug)]
pub struct ShotClock {
    pub next_shot_id: u64,
    pub cooldown: Timer,
}

#[derive(Resource, Debug, Default)]
pub struct ScoreBoard {
    pub shots: u64,
    pub hits: u64,
    pub total_damage: f32,
    pub kill_time_s: Option<f64>,
    /// Manual mode starts this clock on the first emitted shot. Benchmark mode resets it at Running.
    pub trial_start_s: Option<f64>,
    pub peak_rolling_dps: f32,
    hit_history: VecDeque<(f64, f32)>,
    counted_shots: HashSet<u64>,
}

impl ScoreBoard {
    pub fn reset(&mut self, now: f64) {
        self.shots = 0;
        self.hits = 0;
        self.total_damage = 0.0;
        self.kill_time_s = None;
        self.trial_start_s = Some(now);
        self.peak_rolling_dps = 0.0;
        self.hit_history.clear();
        self.counted_shots.clear();
    }

    pub fn reset_manual(&mut self) {
        self.shots = 0;
        self.hits = 0;
        self.total_damage = 0.0;
        self.kill_time_s = None;
        self.trial_start_s = None;
        self.peak_rolling_dps = 0.0;
        self.hit_history.clear();
        self.counted_shots.clear();
    }

    pub fn note_shot(&mut self, now: f64) {
        if self.trial_start_s.is_none() {
            self.trial_start_s = Some(now);
        }
        self.shots += 1;
    }

    pub fn note_hit(&mut self, shot_id: u64, now: f64, damage: f32, dps_window_s: f64) -> bool {
        if !self.counted_shots.insert(shot_id) {
            return false;
        }
        self.hits += 1;
        self.total_damage += damage;
        self.hit_history.push_back((now, damage));
        let dps = self.rolling_dps(now, dps_window_s);
        self.peak_rolling_dps = self.peak_rolling_dps.max(dps);
        true
    }

    pub fn rolling_dps(&mut self, now: f64, window_s: f64) -> f32 {
        let window_s = window_s.max(0.05);
        while self
            .hit_history
            .front()
            .is_some_and(|(t, _)| now - *t > window_s)
        {
            self.hit_history.pop_front();
        }
        self.hit_history.iter().map(|(_, d)| *d).sum::<f32>() / window_s as f32
    }

    pub fn hit_rate_pct(&self) -> f32 {
        if self.shots == 0 {
            0.0
        } else {
            self.hits as f32 * 100.0 / self.shots as f32
        }
    }

    pub fn average_dps(&self, now: f64) -> f32 {
        let Some(t0) = self.trial_start_s else {
            return 0.0;
        };
        let elapsed = (now - t0).max(0.0);
        if self.shots == 0 || elapsed <= 1e-6 {
            0.0
        } else {
            self.total_damage / elapsed as f32
        }
    }
}

#[derive(Resource, Debug, Clone, Copy, Default)]
pub struct CameraIntrinsics {
    pub width: u32,
    pub height: u32,
    pub fx: f64,
    pub fy: f64,
    pub cx: f64,
    pub cy: f64,
}

#[derive(Resource, Debug, Clone, Copy, Default)]
pub struct EvaluationGate {
    pub benchmark_active: bool,
    pub accepting: bool,
    pub trial_id: u64,
    pub rounds_limit: u32,
}
