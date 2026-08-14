use std::{fs, path::Path};

use anyhow::{Context, Result};
use bevy::prelude::Resource;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TargetPath {
    Stationary,
    LineX,
    LineZ,
    Ellipse,
    FigureEight,
}

impl Default for TargetPath {
    fn default() -> Self {
        Self::LineX
    }
}

impl TargetPath {
    pub const ALL: [Self; 5] = [
        Self::Stationary,
        Self::LineX,
        Self::LineZ,
        Self::Ellipse,
        Self::FigureEight,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Stationary => "静止",
            Self::LineX => "沿 X 轴往复",
            Self::LineZ => "沿 Z 轴往复",
            Self::Ellipse => "椭圆轨迹",
            Self::FigureEight => "8 字轨迹",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct WindowConfig {
    pub width: u32,
    pub height: u32,
    pub title: String,
}

impl Default for WindowConfig {
    fn default() -> Self {
        Self {
            width: 1440,
            height: 1080,
            title: "RoboMaster Aim Benchmark Simulator".into(),
        }
    }
}


#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct PhysicsConfig {
    /// Fixed physics update frequency used by Avian/Bevy.
    pub fixed_hz: f32,
    /// Downward gravitational acceleration magnitude.
    pub gravity_mps2: f32,
}

impl Default for PhysicsConfig {
    fn default() -> Self {
        Self {
            fixed_hz: 120.0,
            gravity_mps2: 9.80665,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct CameraConfig {
    pub width: u32,
    pub height: u32,
    pub fps: f32,
    pub vertical_fov_deg: f32,
    pub jpeg_quality: u8,
    /// Camera position in the gimbal local frame. The barrel points along local -Z.
    pub right_m: f32,
    pub above_m: f32,
    pub forward_m: f32,
}

impl Default for CameraConfig {
    fn default() -> Self {
        Self {
            width: 1280,
            height: 720,
            fps: 60.0,
            vertical_fov_deg: 60.0,
            jpeg_quality: 90,
            right_m: 0.0,
            above_m: 0.045,
            forward_m: -0.10,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ShooterConfig {
    pub gimbal_height_m: f32,
    pub barrel_length_m: f32,
    pub barrel_radius_m: f32,
    pub muzzle_offset_m: f32,
    pub yaw_limit_deg: f32,
    pub pitch_min_deg: f32,
    pub pitch_max_deg: f32,
    pub max_yaw_speed_dps: f32,
    pub max_pitch_speed_dps: f32,
}

impl Default for ShooterConfig {
    fn default() -> Self {
        Self {
            gimbal_height_m: 1.10,
            barrel_length_m: 0.55,
            barrel_radius_m: 0.018,
            muzzle_offset_m: 0.55,
            yaw_limit_deg: 180.0,
            pitch_min_deg: -35.0,
            pitch_max_deg: 30.0,
            max_yaw_speed_dps: 720.0,
            max_pitch_speed_dps: 540.0,
        }
    }
}


#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct OperatorConfig {
    /// Horizontal chassis translation speed for W/A/S/D.
    pub chassis_move_speed_mps: f32,
    /// Mouse sensitivity in degrees per raw mouse-motion unit.
    pub mouse_sensitivity_yaw_deg: f32,
    pub mouse_sensitivity_pitch_deg: f32,
    /// When true, W/A/S/D movement is expressed in the current gimbal-yaw frame.
    pub move_relative_to_gimbal: bool,
    /// Native auto-aim command freshness timeout. Stale commands can never fire.
    pub command_timeout_s: f64,
    /// Capture/lock the pointer at startup. Keep this false to require F1 before robot control.
    pub cursor_grab_on_start: bool,
    /// Automated benchmark emulates holding RMB + LMB so it can run unattended.
    pub benchmark_auto_hold_inputs: bool,
}

impl Default for OperatorConfig {
    fn default() -> Self {
        Self {
            chassis_move_speed_mps: 2.5,
            mouse_sensitivity_yaw_deg: 0.12,
            mouse_sensitivity_pitch_deg: 0.10,
            move_relative_to_gimbal: true,
            command_timeout_s: 0.35,
            cursor_grab_on_start: false,
            benchmark_auto_hold_inputs: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct TargetConfig {
    pub initial_distance_m: f32,
    pub armor_center_height_m: f32,
    /// Nominal overall width of the AM02 module, including its side indicators.
    pub armor_width_m: f32,
    /// Height of the AM02 impact module.
    pub armor_height_m: f32,
    /// Overall module depth used by the collision shape.
    pub armor_thickness_m: f32,
    /// Width of the main impact plate inside the complete module.
    pub armor_face_width_m: f32,
    /// Width of one side indicator light guide.
    pub armor_light_width_m: f32,
    /// Height of one side indicator light guide.
    pub armor_light_height_m: f32,
    /// Width of the illuminated core inside the light guide.
    pub armor_light_emissive_width_m: f32,
    pub front_back_radius_m: f32,
    pub left_right_radius_m: f32,
    pub rpm: f32,
    pub path: TargetPath,
    pub translation_speed_mps: f32,
    pub half_extent_x_m: f32,
    pub half_extent_z_m: f32,
    pub max_hp: f32,
    pub damage_per_hit: f32,
    pub freeze_when_dead: bool,
}

impl Default for TargetConfig {
    fn default() -> Self {
        Self {
            initial_distance_m: 5.0,
            armor_center_height_m: 0.50,
            armor_width_m: 0.140,
            armor_height_m: 0.125,
            armor_thickness_m: 0.012,
            armor_face_width_m: 0.135,
            armor_light_width_m: 0.012,
            armor_light_height_m: 0.059,
            armor_light_emissive_width_m: 0.007,
            front_back_radius_m: 0.280,
            left_right_radius_m: 0.280,
            rpm: 60.0,
            path: TargetPath::LineX,
            translation_speed_mps: 1.0,
            half_extent_x_m: 2.0,
            half_extent_z_m: 1.0,
            max_hp: 500.0,
            damage_per_hit: 10.0,
            freeze_when_dead: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ProjectileConfig {
    pub diameter_m: f32,
    pub speed_mps: f32,
    pub mass_kg: f32,
    pub cooldown_s: f32,
    pub lifetime_s: f32,
    pub linear_damping: f32,
}

impl Default for ProjectileConfig {
    fn default() -> Self {
        Self {
            diameter_m: 0.017,
            speed_mps: 15.0,
            mass_kg: 0.0032,
            cooldown_s: 0.08,
            lifetime_s: 3.0,
            linear_damping: 0.0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct NetworkConfig {
    /// UDP socket on which the simulator receives JSON GimbalCommand packets.
    pub command_bind: String,
    /// UDP destination for JSON Telemetry packets.
    pub telemetry_target: String,
    /// TCP listener for JPEG camera frames.
    pub camera_bind: String,
    pub telemetry_hz: f32,
}

impl Default for NetworkConfig {
    fn default() -> Self {
        Self {
            command_bind: "127.0.0.1:39000".into(),
            telemetry_target: "127.0.0.1:39001".into(),
            camera_bind: "127.0.0.1:39002".into(),
            telemetry_hz: 120.0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct BenchmarkConfig {
    pub distances_m: Vec<f32>,
    pub rpms: Vec<f32>,
    pub translation_speeds_mps: Vec<f32>,
    pub rounds_per_trial: u32,
    pub repeats_per_condition: u32,
    pub warmup_s: f64,
    pub case_timeout_s: f64,
    pub post_fire_grace_s: f64,
    pub dps_window_s: f64,
    pub output_dir: String,
    pub autostart: bool,
}

impl Default for BenchmarkConfig {
    fn default() -> Self {
        Self {
            distances_m: vec![3.0, 5.0, 7.0, 10.0],
            rpms: vec![0.0, 30.0, 60.0, 120.0, 180.0],
            translation_speeds_mps: vec![0.0, 1.0, 2.0, 3.0],
            rounds_per_trial: 100,
            repeats_per_condition: 1,
            warmup_s: 1.0,
            case_timeout_s: 20.0,
            post_fire_grace_s: 1.2,
            dps_window_s: 1.0,
            output_dir: "benchmark_results".into(),
            autostart: false,
        }
    }
}

#[derive(Resource, Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct SimConfig {
    pub window: WindowConfig,
    pub physics: PhysicsConfig,
    pub camera: CameraConfig,
    pub shooter: ShooterConfig,
    pub operator: OperatorConfig,
    pub target: TargetConfig,
    pub projectile: ProjectileConfig,
    pub network: NetworkConfig,
    pub benchmark: BenchmarkConfig,
}

impl SimConfig {
    pub fn load(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        let text = fs::read_to_string(path)
            .with_context(|| format!("failed to read config {}", path.display()))?;
        toml::from_str(&text).with_context(|| format!("invalid TOML in {}", path.display()))
    }
}
