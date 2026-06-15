use chrono::{DateTime, Utc};
use clickhouse::Row;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Bell {
    pub bell_id: Uuid,
    pub bell_name: String,
    pub dynasty: String,
    pub bell_type: String,
    pub material: String,
    pub height_m: f64,
    pub diameter_m: f64,
    pub weight_kg: f64,
    pub expected_pitch: String,
    pub expected_freq_hz: f64,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Row)]
pub struct SensorReading {
    pub reading_id: Uuid,
    pub bell_id: Uuid,
    pub timestamp: DateTime<Utc>,
    pub temp_celsius: f64,
    pub temp_gradient: f64,
    pub wall_thickness_mm: f64,
    pub thickness_deviation: f64,
    pub alloy_cu: f64,
    pub alloy_sn: f64,
    pub alloy_pb: f64,
    pub alloy_zn: f64,
    pub alloy_other: f64,
    pub acoustic_freq_hz: f64,
    pub acoustic_amplitude: f64,
    pub acoustic_decay: f64,
    pub acoustic_harmonics: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SensorReadingIn {
    pub bell_id: Uuid,
    pub temp_celsius: f64,
    pub temp_gradient: f64,
    pub wall_thickness_mm: f64,
    pub thickness_deviation: f64,
    pub alloy_cu: f64,
    pub alloy_sn: f64,
    pub alloy_pb: f64,
    pub alloy_zn: f64,
    pub alloy_other: f64,
    pub acoustic_freq_hz: f64,
    pub acoustic_amplitude: f64,
    pub acoustic_decay: f64,
    pub acoustic_harmonics: Vec<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Row)]
pub struct CastingSimulation {
    pub sim_id: Uuid,
    pub bell_id: Uuid,
    pub timestamp: DateTime<Utc>,
    pub sim_type: String,
    pub time_step_sec: u32,
    pub temp_field: String,
    pub solid_fraction: String,
    pub shrinkage_porosity: String,
    pub defect_locations: String,
    pub defect_count: u32,
    pub max_shrinkage: f64,
    pub cooling_rate: f64,
    pub prediction_risk: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CastingSimRequest {
    pub bell_id: Uuid,
    pub sim_type: String,
    pub initial_temp: f64,
    pub grid_size: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Row)]
pub struct AcousticSimulation {
    pub sim_id: Uuid,
    pub bell_id: Uuid,
    pub timestamp: DateTime<Utc>,
    pub method: String,
    pub natural_frequencies: String,
    pub mode_shapes: String,
    pub far_field_pressure: String,
    pub sound_field_2d: String,
    pub directivity_index: f64,
    pub sound_power: f64,
    pub pitch_deviation_cents: f64,
    pub pitch_ok: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AcousticSimRequest {
    pub bell_id: Uuid,
    pub method: String,
    pub young_modulus: Option<f64>,
    pub poisson_ratio: Option<f64>,
    pub density: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Row)]
pub struct Alert {
    pub alert_id: Uuid,
    pub bell_id: Uuid,
    pub timestamp: DateTime<Utc>,
    pub alert_type: String,
    pub severity: String,
    pub message: String,
    pub related_reading: Option<Uuid>,
    pub related_sim: Option<Uuid>,
    pub resolved: bool,
    pub resolved_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Row)]
pub struct CastingProcess {
    pub process_id: Uuid,
    pub bell_id: Uuid,
    pub timestamp: DateTime<Utc>,
    pub stage: String,
    pub progress: f64,
    pub current_temp: f64,
    pub mold_fill_level: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiResponse<T: Serialize> {
    pub success: bool,
    pub data: Option<T>,
    pub error: Option<String>,
}

impl<T: Serialize> ApiResponse<T> {
    pub fn ok(data: T) -> Self {
        Self {
            success: true,
            data: Some(data),
            error: None,
        }
    }

    pub fn err(msg: impl Into<String>) -> Self {
        Self {
            success: false,
            data: None,
            error: Some(msg.into()),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MqttAlertPayload {
    pub alert_id: Uuid,
    pub bell_id: Uuid,
    pub bell_name: String,
    pub timestamp: DateTime<Utc>,
    pub alert_type: String,
    pub severity: String,
    pub message: String,
}
