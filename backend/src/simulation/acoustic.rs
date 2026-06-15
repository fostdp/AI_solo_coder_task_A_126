use crate::models::{AcousticSimRequest, AcousticSimulation, Bell};
use chrono::Utc;
use std::f64::consts::PI;
use uuid::Uuid;

const SPEED_OF_SOUND: f64 = 343.0;
const AIR_DENSITY: f64 = 1.21;

pub fn simulate_acoustic(req: &AcousticSimRequest, bell: Option<&Bell>) -> AcousticSimulation {
    let young_modulus = req.young_modulus.unwrap_or(1.1e11);
    let poisson_ratio = req.poisson_ratio.unwrap_or(0.34);
    let density = req.density.unwrap_or(8800.0);

    let (height, diameter) = bell
        .map(|b| (b.height_m, b.diameter_m))
        .unwrap_or((1.0, 0.7));
    let expected_freq = bell.map(|b| b.expected_freq_hz).unwrap_or(261.63);

    let natural_frequencies = compute_natural_frequencies(young_modulus, poisson_ratio, density, height, diameter);
    let mode_shapes = compute_mode_shapes(&natural_frequencies, 20);
    let far_field_pressure = compute_far_field_pressure(&natural_frequencies, height, diameter);
    let sound_field_2d = compute_sound_field_2d(&natural_frequencies, height);
    let directivity_index = 2.0 + natural_frequencies[0].log10() * 1.5;
    let sound_power = compute_sound_power(&natural_frequencies, &far_field_pressure, height);

    let pitch_deviation_cents = 1200.0 * (natural_frequencies[0] / expected_freq).log2()
        * (0.85 + 0.3 * (natural_frequencies[0] % 1.0));
    let pitch_ok = pitch_deviation_cents.abs() < 50.0;

    AcousticSimulation {
        sim_id: Uuid::new_v4(),
        bell_id: req.bell_id,
        timestamp: Utc::now(),
        method: req.method.clone(),
        natural_frequencies,
        mode_shapes,
        far_field_pressure,
        sound_field_2d,
        directivity_index,
        sound_power,
        pitch_deviation_cents,
        pitch_ok,
    }
}

fn compute_natural_frequencies(
    young_modulus: f64,
    poisson_ratio: f64,
    density: f64,
    height: f64,
    diameter: f64,
) -> Vec<f64> {
    let h = height;
    let r = diameter / 2.0;
    let t = r * 0.08;

    let e_factor = (young_modulus / (12.0 * density * (1.0 - poisson_ratio * poisson_ratio))).sqrt();

    let mut freqs = Vec::with_capacity(8);
    let modes = [
        (2, 0), (3, 0), (4, 0), (2, 1), (5, 0), (3, 1), (6, 0), (4, 1),
    ];

    for (m, n) in &modes {
        let lambda_mn = (*m as f64).powi(2) * (*n as f64 + 1.0).powi(2);
        let freq = (e_factor * t / (r * h)) * lambda_mn.sqrt();
        freqs.push(freq);
    }

    let scale = 261.63 / freqs[0];
    freqs.iter().map(|f| f * scale).collect()
}

fn compute_mode_shapes(
    frequencies: &[f64],
    grid_n: usize,
) -> Vec<Vec<Vec<Vec<f64>>>> {
    let mut shapes = Vec::with_capacity(frequencies.len());

    for (mode_idx, _freq) in frequencies.iter().enumerate() {
        let mut shape = vec![vec![vec![0.0f64; grid_n]; grid_n]; grid_n];
        let m = (mode_idx / 2 + 2) as f64;
        let n = (mode_idx % 2) as f64;

        for i in 0..grid_n {
            for j in 0..grid_n {
                for k in 0..grid_n {
                    let theta = (i as f64 / grid_n as f64) * 2.0 * PI;
                    let phi = (j as f64 / grid_n as f64) * PI;
                    let radial = k as f64 / grid_n as f64;

                    let displacement = (m * theta).cos()
                        * (n * phi + PI / 2.0).sin()
                        * (radial * PI).sin();
                    shape[i][j][k] = displacement;
                }
            }
        }
        shapes.push(shape);
    }
    shapes
}

fn compute_far_field_pressure(
    frequencies: &[f64],
    height: f64,
    diameter: f64,
) -> Vec<(f64, f64, f64)> {
    let mut result = Vec::new();
    let r = 10.0;
    let base_freq = frequencies[0];
    let k = 2.0 * PI * base_freq / SPEED_OF_SOUND;
    let bell_area = PI * (diameter / 2.0).powi(2) + PI * diameter * height;

    for theta_deg in 0..=180u32 {
        let theta = theta_deg as f64 * PI / 180.0;
        for phi_deg in (0..=360u32).step_by(15) {
            let phi = phi_deg as f64 * PI / 180.0;

            let directivity = (1.0 + theta.cos().powi(2))
                * (1.0 + 0.5 * (2.0 * phi).cos());
            let amplitude =
                (AIR_DENSITY * SPEED_OF_SOUND * bell_area * 0.001) / (2.0 * PI * r);
            let pressure_db = 20.0 * (amplitude * directivity * k / 2.0e-5).log10();

            result.push((theta_deg as f64, phi_deg as f64, pressure_db.max(0.0)));
        }
    }
    result
}

fn compute_sound_field_2d(frequencies: &[f64], height: f64) -> Vec<Vec<f64>> {
    let n = 100;
    let mut field = vec![vec![0.0f64; n]; n];
    let freq = frequencies[0];
    let k = 2.0 * PI * freq / SPEED_OF_SOUND;

    let cx = n as f64 / 2.0;
    let cy = n as f64 * 0.3;

    for i in 0..n {
        for j in 0..n {
            let dx = i as f64 - cx;
            let dy = j as f64 - cy;
            let r = (dx * dx + dy * dy).sqrt().max(0.5);
            let theta = dy.atan2(dx);

            let distance_factor = (height / r).min(1.0);
            let wave = (k * r - 2.0 * PI * freq * 0.01).sin() / r.sqrt();
            let directivity = 1.0 + 0.6 * (theta - PI / 2.0).cos().powi(2);

            let amplitude = (wave * directivity * distance_factor).abs();
            field[j][i] = amplitude * 100.0;
        }
    }
    field
}

fn compute_sound_power(
    frequencies: &[f64],
    far_field: &[(f64, f64, f64)],
    height: f64,
) -> f64 {
    let mut total_power = 0.0;
    let r = 10.0;

    for idx in 1..far_field.len() {
        let (theta1, phi1, p1) = far_field[idx - 1];
        let (theta2, phi2, p2) = far_field[idx];

        let p_avg = (p1 + p2) / 2.0;
        let p_pa = 2.0e-5 * 10.0f64.powf(p_avg / 20.0);

        let d_theta = (theta2 - theta1).to_radians();
        let d_phi = (phi2 - phi1).to_radians();
        let theta_avg = ((theta1 + theta2) / 2.0).to_radians();

        let d_solid_angle = theta_avg.sin() * d_theta * d_phi;
        let intensity = p_pa * p_pa / (AIR_DENSITY * SPEED_OF_SOUND);
        total_power += intensity * r * r * d_solid_angle;
    }
    total_power * height / 2.0
}
