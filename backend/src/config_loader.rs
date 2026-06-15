use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

pub static MATERIALS: Lazy<HashMap<String, Material>> = Lazy::new(|| {
    let data = include_str!("../../config/materials.json");
    serde_json::from_str::<HashMap<String, Material>>(data)
        .expect("Failed to load materials.json")
});

pub static ACOUSTIC_PARAMS: Lazy<AcousticParams> = Lazy::new(|| {
    let data = include_str!("../../config/acoustic_params.json");
    serde_json::from_str::<AcousticParams>(data)
        .expect("Failed to load acoustic_params.json")
});

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Material {
    pub name: String,
    pub density: f64,
    pub young_modulus: f64,
    pub poisson_ratio: f64,
    pub thermal_conductivity: f64,
    pub thermal_diffusivity: f64,
    pub specific_heat: f64,
    pub melting_point: f64,
    pub liquidus_temperature: f64,
    pub solidus_temperature: f64,
    pub shrinkage_coefficient: f64,
    pub niyama_critical: f64,
    pub niyama_high_risk: f64,
    pub composition: HashMap<String, f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AcousticParams {
    pub air: AirParams,
    pub bell_acoustics: BellAcoustics,
    pub bem_solver: BEMSolver,
    pub sound_field_visualization: SoundFieldVis,
    pub alert_thresholds: AlertThresholds,
    pub modal_analysis: ModalAnalysis,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AirParams {
    pub speed_of_sound: f64,
    pub density: f64,
    pub impedance: f64,
    pub attenuation_coeff: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BellAcoustics {
    pub reference_pressure: f64,
    pub pitch_tolerance_cents: f64,
    pub pitch_warning_cents: f64,
    pub pitch_critical_cents: f64,
    pub mode_frequency_ratios: Vec<ModeRatio>,
    pub decay_time_range: Range,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModeRatio {
    pub mode: String,
    pub ratio: f64,
    pub weight: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Range {
    pub min: f64,
    pub max: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BEMSolver {
    pub min_panels: usize,
    pub max_panels: usize,
    pub default_panels: usize,
    pub tikhonov_alpha_min: f64,
    pub tikhonov_alpha_max: f64,
    pub low_frequency_threshold_hz: f64,
    pub far_field_distance_m: f64,
    pub integration_points: usize,
    pub singular_treatment: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SoundFieldVis {
    pub min_frequency_hz: f64,
    pub max_frequency_hz: f64,
    pub default_wave_speed: f64,
    pub colormap_hue_range: HueRange,
    pub animation_interval_ms: u64,
    pub high_dpi_scale: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HueRange {
    pub min: f64,
    pub max: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlertThresholds {
    pub temperature_max_celsius: f64,
    pub thickness_deviation_warning_pct: f64,
    pub thickness_deviation_danger_pct: f64,
    pub alloy_sum_tolerance_pct: f64,
    pub cooling_rate_normal_min_cps: f64,
    pub cooling_rate_normal_max_cps: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModalAnalysis {
    pub min_mode_order: u32,
    pub max_mode_order: u32,
    pub damping_coefficient: f64,
    pub poisson_correction_factor: f64,
    pub thickness_correction_exponent: f64,
}

pub fn get_material(key: &str) -> Option<&'static Material> {
    MATERIALS.get(key)
}

pub fn get_default_material() -> &'static Material {
    MATERIALS.get("bronze_qing_qin").unwrap_or_else(|| MATERIALS.values().next().unwrap())
}

pub fn get_material_for_bell(bell_type: &str) -> &'static Material {
    if bell_type.contains("编钟") {
        get_material("bronze_qing_qin").unwrap_or_else(get_default_material)
    } else if bell_type.contains("朝钟") || bell_type.contains("永乐") {
        get_material("bronze_yong_le").unwrap_or_else(get_default_material)
    } else if bell_type.contains("佛钟") {
        get_material("bronze_fo_zhong").unwrap_or_else(get_default_material)
    } else {
        get_default_material()
    }
}
