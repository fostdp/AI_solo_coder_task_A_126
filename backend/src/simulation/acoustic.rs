use crate::config_loader::{self, ACOUSTIC_PARAMS, Material};
use crate::models::{AcousticSimRequest, AcousticSimulation, Bell};
use chrono::Utc;
use std::f64::consts::PI;
use uuid::Uuid;

pub fn simulate_acoustic(req: &AcousticSimRequest, bell: Option<&Bell>, material: &Material) -> AcousticSimulation {
    let young_modulus = req.young_modulus.unwrap_or(material.young_modulus);
    let poisson_ratio = req.poisson_ratio.unwrap_or(material.poisson_ratio);
    let density = req.density.unwrap_or(material.density);

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

    let pitch_deviation_cents = 1200.0 * (natural_frequencies[0] / expected_freq).log2();
    let pitch_ok = pitch_deviation_cents.abs() < ACOUSTIC_PARAMS.bell_acoustics.pitch_tolerance_cents;

    AcousticSimulation {
        sim_id: Uuid::new_v4(),
        bell_id: req.bell_id,
        timestamp: Utc::now(),
        method: req.method.clone(),
        natural_frequencies: serde_json::to_string(&natural_frequencies).unwrap_or_default(),
        mode_shapes: serde_json::to_string(&mode_shapes).unwrap_or_default(),
        far_field_pressure: serde_json::to_string(&far_field_pressure).unwrap_or_default(),
        sound_field_2d: serde_json::to_string(&sound_field_2d).unwrap_or_default(),
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

    freqs
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

fn compute_tikhonov_alpha(freq: f64, cond_est: f64) -> f64 {
    let alpha_min = ACOUSTIC_PARAMS.bem_solver.tikhonov_alpha_min;
    let alpha_max = ACOUSTIC_PARAMS.bem_solver.tikhonov_alpha_max;
    let low_freq = ACOUSTIC_PARAMS.bem_solver.low_frequency_threshold_hz;

    let freq_factor = if freq < low_freq {
        let ratio = (low_freq - freq) / low_freq;
        1.0 + ratio * 100.0
    } else {
        (low_freq / freq).min(1.0)
    };

    let cond_factor = if cond_est > 1e6 {
        cond_est.log10() / 6.0
    } else {
        1.0
    };

    (alpha_max * freq_factor * cond_factor).min(alpha_max).max(alpha_min)
}

fn estimate_condition_number(matrix: &[Vec<f64>]) -> f64 {
    let n = matrix.len();
    let mut max_row_sum = 0.0f64;
    let mut min_diag = f64::INFINITY;

    for i in 0..n {
        let mut row_sum = 0.0;
        for j in 0..n {
            row_sum += matrix[i][j].abs();
            if i == j && matrix[i][j].abs() > 1e-10 {
                min_diag = min_diag.min(matrix[i][j].abs());
            }
        }
        max_row_sum = max_row_sum.max(row_sum);
    }

    if min_diag < 1e-10 {
        1e12
    } else {
        max_row_sum / min_diag
    }
}

fn mat_transpose(a: &[Vec<f64>]) -> Vec<Vec<f64>> {
    let n = a.len();
    let m = a[0].len();
    let mut at = vec![vec![0.0f64; n]; m];
    for i in 0..n {
        for j in 0..m {
            at[j][i] = a[i][j];
        }
    }
    at
}

fn mat_mul(a: &[Vec<f64>], b: &[Vec<f64>]) -> Vec<Vec<f64>> {
    let n = a.len();
    let m = b[0].len();
    let k = b.len();
    let mut c = vec![vec![0.0f64; m]; n];
    for i in 0..n {
        for l in 0..k {
            let ai = a[i][l];
            if ai.abs() > 1e-10 {
                for j in 0..m {
                    c[i][j] += ai * b[l][j];
                }
            }
        }
    }
    c
}

fn mat_vec_mul(a: &[Vec<f64>], x: &[f64]) -> Vec<f64> {
    let n = a.len();
    let m = a[0].len();
    let mut b = vec![0.0f64; n];
    for i in 0..n {
        for j in 0..m {
            b[i] += a[i][j] * x[j];
        }
    }
    b
}

fn solve_cholesky(a: &mut [Vec<f64>], b: &[f64]) -> Option<Vec<f64>> {
    let n = a.len();
    let mut l = vec![vec![0.0f64; n]; n];

    for i in 0..n {
        for j in 0..=i {
            let mut sum = a[i][j];
            for k in 0..j {
                sum -= l[i][k] * l[j][k];
            }
            if i == j {
                if sum <= 0.0 {
                    return None;
                }
                l[i][j] = sum.sqrt();
            } else {
                l[i][j] = sum / l[j][j];
            }
        }
    }

    let mut y = vec![0.0f64; n];
    for i in 0..n {
        let mut sum = b[i];
        for k in 0..i {
            sum -= l[i][k] * y[k];
        }
        y[i] = sum / l[i][i];
    }

    let mut x = vec![0.0f64; n];
    for i in (0..n).rev() {
        let mut sum = y[i];
        for k in i + 1..n {
            sum -= l[k][i] * x[k];
        }
        x[i] = sum / l[i][i];
    }

    Some(x)
}

fn solve_tikhonov(
    a: &[Vec<f64>],
    b: &[f64],
    alpha: f64,
) -> Vec<f64> {
    let n = a.len();
    let at = mat_transpose(a);
    let ata = mat_mul(&at, a);

    let mut m = vec![vec![0.0f64; n]; n];
    for i in 0..n {
        for j in 0..n {
            m[i][j] = ata[i][j] + if i == j { alpha } else { 0.0 };
        }
    }

    let atb = mat_vec_mul(&at, b);

    if let Some(x) = solve_cholesky(&mut m, &atb) {
        x
    } else {
        let mut result = vec![0.0f64; n];
        for i in 0..n {
            result[i] = atb[i] / (m[i][i] + 1e-6);
        }
        result
    }
}

fn construct_bem_matrix(
    panels: &[(f64, f64, f64, f64, f64, f64)],
    k: f64,
) -> Vec<Vec<f64>> {
    let n = panels.len();
    let mut a = vec![vec![0.0f64; n]; n];
    let eps = 1e-10;

    for i in 0..n {
        let (xi, yi, zi, _, _, _) = panels[i];
        for j in 0..n {
            let (xj, yj, zj, nxj, nyj, nzj) = panels[j];
            let dx = xi - xj;
            let dy = yi - yj;
            let dz = zi - zj;
            let r = (dx * dx + dy * dy + dz * dz).sqrt().max(eps);

            if i == j {
                a[i][j] = 0.5;
            } else {
                let r_dot_n = dx * nxj + dy * nyj + dz * nzj;
                let ikr = k * r;
                let g = (0.0 + ikr).exp() / (4.0 * PI * r);
                let dg_dn = g * r_dot_n * (-1.0 / r + k * 0.01);
                a[i][j] = dg_dn;
            }
        }
    }
    a
}

fn construct_bem_rhs(
    panels: &[(f64, f64, f64, f64, f64, f64)],
    k: f64,
    base_amp: f64,
) -> Vec<f64> {
    let n = panels.len();
    let mut b = vec![0.0f64; n];

    for i in 0..n {
        let (xi, yi, zi, _, _, _) = panels[i];
        let r_from_top = (xi * xi + yi * yi + (zi + 1.0) * (zi + 1.0)).sqrt().max(1e-10);
        let incident = base_amp * (0.0 + k * r_from_top).exp() / r_from_top;
        b[i] = incident;
    }
    b
}

fn generate_bell_panels(height: f64, diameter: f64, n_panels: usize) -> Vec<(f64, f64, f64, f64, f64, f64)> {
    let n_theta = (n_panels as f64).sqrt().round() as usize;
    let n_phi = n_theta * 2;
    let r = diameter / 2.0;
    let h = height;

    let mut panels = Vec::with_capacity(n_theta * n_phi);
    for i in 0..n_theta {
        let t = i as f64 / (n_theta - 1) as f64;
        let y = h / 2.0 - t * h;
        let radius = if t < 0.15 {
            r * 0.6 + (r * 0.8 - r * 0.6) * (t / 0.15)
        } else if t < 0.7 {
            let lt = (t - 0.15) / 0.55;
            r * 0.8 * (1.0 + 0.08 * (lt * PI).sin())
        } else {
            let lt = (t - 0.7) / 0.3;
            r * 0.8 + (r - r * 0.8) * (1.0 - (lt * PI / 2.0).cos())
        };

        for j in 0..n_phi {
            let phi = (j as f64 / n_phi as f64) * 2.0 * PI;
            let x = radius * phi.cos();
            let z = radius * phi.sin();

            let nx = phi.cos();
            let ny = 0.1;
            let nz = phi.sin();
            let n_len = (nx * nx + ny * ny + nz * nz).sqrt();

            panels.push((
                x, y, z,
                nx / n_len, ny / n_len, nz / n_len,
            ));
        }
    }
    panels
}

fn compute_far_field_pressure(
    frequencies: &[f64],
    height: f64,
    diameter: f64,
) -> Vec<(f64, f64, f64)> {
    let mut result = Vec::new();
    let r = ACOUSTIC_PARAMS.bem_solver.far_field_distance_m;
    let base_freq = frequencies[0];
    let speed_of_sound = ACOUSTIC_PARAMS.air.speed_of_sound;
    let air_density = ACOUSTIC_PARAMS.air.density;
    let low_freq = ACOUSTIC_PARAMS.bem_solver.low_frequency_threshold_hz;
    let p_ref = ACOUSTIC_PARAMS.bell_acoustics.reference_pressure;
    let k = 2.0 * PI * base_freq / speed_of_sound;

    let n_panels = ACOUSTIC_PARAMS.bem_solver.default_panels;
    let panels = generate_bell_panels(height, diameter, n_panels);
    let bell_area = PI * (diameter / 2.0).powi(2) + PI * diameter * height;

    let bem_matrix = construct_bem_matrix(&panels, k);
    let cond_est = estimate_condition_number(&bem_matrix);
    let alpha = compute_tikhonov_alpha(base_freq, cond_est);

    let base_amp = air_density * speed_of_sound * bell_area * 0.001 / (2.0 * PI * r);
    let rhs = construct_bem_rhs(&panels, k, base_amp);
    let surface_pressure = solve_tikhonov(&bem_matrix, &rhs, alpha);

    let mut avg_surface_p = 0.0;
    for &p in &surface_pressure {
        avg_surface_p += p.abs();
    }
    avg_surface_p /= surface_pressure.len() as f64;

    for theta_deg in 0..=180u32 {
        let theta = theta_deg as f64 * PI / 180.0;
        for phi_deg in (0..=360u32).step_by(15) {
            let phi = phi_deg as f64 * PI / 180.0;

            let directivity = (1.0 + theta.cos().powi(2))
                * (1.0 + 0.5 * (2.0 * phi).cos());

            let reg_factor = if base_freq < low_freq {
                0.7 + 0.3 * (base_freq / low_freq)
            } else {
                1.0
            };

            let amplitude = avg_surface_p * reg_factor / r;
            let pressure = amplitude * directivity;
            let pressure_db = 20.0 * (pressure.abs().max(p_ref) / p_ref).log10();

            result.push((theta_deg as f64, phi_deg as f64, pressure_db.max(0.0)));
        }
    }

    if base_freq < low_freq {
        tracing::debug!(
            "[BEM] freq={:.1}Hz, cond≈{:.2e}, α={:.2e}, panels={}",
            base_freq, cond_est, alpha, n_panels
        );
    }

    result
}

fn compute_sound_field_2d(frequencies: &[f64], height: f64) -> Vec<Vec<f64>> {
    let n = 100;
    let mut field = vec![vec![0.0f64; n]; n];
    let freq = frequencies[0];
    let k = 2.0 * PI * freq / ACOUSTIC_PARAMS.air.speed_of_sound;

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
        let intensity = p_pa * p_pa / (ACOUSTIC_PARAMS.air.density * ACOUSTIC_PARAMS.air.speed_of_sound);
        total_power += intensity * r * r * d_solid_angle;
    }
    total_power * height / 2.0
}
