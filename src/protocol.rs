use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GimbalCommand {
    pub yaw_deg: f32,
    pub pitch_deg: f32,
    #[serde(default)]
    pub fire: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PoseWire {
    pub translation_m: [f32; 3],
    /// Quaternion in [x, y, z, w].
    pub quaternion_xyzw: [f32; 4],
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CameraInfoWire {
    pub width: u32,
    pub height: u32,
    pub fx: f64,
    pub fy: f64,
    pub cx: f64,
    pub cy: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TelemetryPacket {
    pub protocol: &'static str,
    pub timestamp_ns: u128,
    pub gimbal_yaw_deg: f32,
    pub gimbal_pitch_deg: f32,
    pub auto_aim_enabled: bool,
    pub operator_trigger_held: bool,
    pub external_fire_advice: bool,
    pub external_command_fresh: bool,
    pub shooter_pose: PoseWire,
    pub gimbal_pose: PoseWire,
    pub muzzle_pose: PoseWire,
    pub camera_pose: PoseWire,
    pub camera_info: CameraInfoWire,
    pub target_hp: f32,
    pub target_max_hp: f32,
    pub target_rpm: f32,
    pub target_translation_speed_mps: f32,
    pub shots: u64,
    pub hits: u64,
    pub hit_rate_pct: f32,
    pub total_damage: f32,
    pub average_dps: f32,
    pub rolling_dps: f32,
}

pub const CAMERA_MAGIC: [u8; 8] = *b"AIMSIM01";
pub const CAMERA_HEADER_BYTES: usize = 40;

#[derive(Debug, Clone)]
pub struct CameraFrame {
    pub frame_id: u64,
    pub timestamp_ns: u64,
    pub width: u32,
    pub height: u32,
    pub jpeg: Vec<u8>,
}

impl CameraFrame {
    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(CAMERA_HEADER_BYTES + self.jpeg.len());
        out.extend_from_slice(&CAMERA_MAGIC);
        out.extend_from_slice(&self.frame_id.to_be_bytes());
        out.extend_from_slice(&self.timestamp_ns.to_be_bytes());
        out.extend_from_slice(&self.width.to_be_bytes());
        out.extend_from_slice(&self.height.to_be_bytes());
        out.extend_from_slice(&(self.jpeg.len() as u32).to_be_bytes());
        out.extend_from_slice(&0u32.to_be_bytes());
        out.extend_from_slice(&self.jpeg);
        out
    }
}
