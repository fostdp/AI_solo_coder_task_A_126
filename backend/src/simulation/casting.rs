use crate::models::{CastingSimRequest, CastingSimulation};
use chrono::Utc;
use rand::Rng;
use std::f64::consts::PI;
use uuid::Uuid;

const TLIQUIDUS: f64 = 1083.0;
const TSOLIDUS: f64 = 950.0;
const AMBIENT_TEMP: f64 = 25.0;
const THERMAL_DIFFUSIVITY: f64 = 1.2e-5;
const SHRINKAGE_COEFF: f64 = 0.045;

const NIYAMA_CRITICAL: f64 = 0.8;
const NIYAMA_HIGH_RISK: f64 = 0.5;
const CELL_SIZE: f64 = 0.002;
const TIME_STEP: f64 = 1.0;

pub fn simulate_casting(req: &CastingSimRequest) -> CastingSimulation {
    let grid_size = req.grid_size.unwrap_or(20);
    let mut rng = rand::thread_rng();

    let temp_field_t0 = compute_temperature_field(req.initial_temp, grid_size, 3500.0);
    let temp_field_t1 = compute_temperature_field(req.initial_temp, grid_size, 3600.0);
    let solid_fraction = compute_solid_fraction(&temp_field_t1);
    let temp_gradient = compute_temperature_gradient(&temp_field_t1);
    let cooling_rate_field = compute_cooling_rate_field(&temp_field_t0, &temp_field_t1);
    let niyama_field = compute_niyama_criterion(&temp_gradient, &cooling_rate_field);
    let shrinkage_porosity = compute_shrinkage_porosity(&solid_fraction, &temp_field_t1, &niyama_field);
    let defect_locations = identify_defects(&shrinkage_porosity, 0.02);
    let max_shrinkage = shrinkage_porosity
        .iter()
        .flatten()
        .flatten()
        .cloned()
        .fold(0.0f64, f64::max);
    let avg_cooling_rate = cooling_rate_field
        .iter()
        .flatten()
        .flatten()
        .cloned()
        .sum::<f64>()
        / (grid_size * grid_size * grid_size) as f64
        * rng.gen_range(0.9..1.1);

    let prediction_risk = if max_shrinkage > 0.08 {
        "critical".to_string()
    } else if max_shrinkage > 0.05 {
        "high".to_string()
    } else if max_shrinkage > 0.02 {
        "medium".to_string()
    } else {
        "low".to_string()
    };

    CastingSimulation {
        sim_id: Uuid::new_v4(),
        bell_id: req.bell_id,
        timestamp: Utc::now(),
        sim_type: req.sim_type.clone(),
        time_step_sec: 3600,
        temp_field,
        solid_fraction,
        shrinkage_porosity,
        defect_locations,
        defect_count: defect_locations.len() as u32,
        max_shrinkage,
        cooling_rate: avg_cooling_rate,
        prediction_risk,
    }
}

fn compute_temperature_field(initial_temp: f64, n: usize, time_sec: f64) -> Vec<Vec<Vec<f64>>> {
    let mut field = vec![vec![vec![0.0f64; n]; n]; n];
    let center = (n as f64) / 2.0;

    for i in 0..n {
        for j in 0..n {
            for k in 0..n {
                let dx = i as f64 - center;
                let dy = j as f64 - center;
                let dz = k as f64 - center;
                let r = (dx * dx + dy * dy + dz * dz).sqrt() / center;

                if r > 1.0 {
                    field[i][j][k] = AMBIENT_TEMP;
                } else {
                    let normalized_r = r.clamp(0.0, 1.0);
                    let cooling_factor = (-normalized_r * normalized_r * time_sec * THERMAL_DIFFUSIVITY / 100.0).exp();
                    let surface_cooling = (1.0 - normalized_r) * 0.3 + 0.7;
                    field[i][j][k] = AMBIENT_TEMP
                        + (initial_temp - AMBIENT_TEMP) * cooling_factor * surface_cooling;
                }
            }
        }
    }
    field
}

fn compute_solid_fraction(temp_field: &[Vec<Vec<f64>>]) -> Vec<Vec<Vec<f64>>> {
    let n = temp_field.len();
    let mut fraction = vec![vec![vec![0.0f64; n]; n]; n];

    for i in 0..n {
        for j in 0..n {
            for k in 0..n {
                let t = temp_field[i][j][k];
                if t >= TLIQUIDUS {
                    fraction[i][j][k] = 0.0;
                } else if t <= TSOLIDUS {
                    fraction[i][j][k] = 1.0;
                } else {
                    let ratio = (TLIQUIDUS - t) / (TLIQUIDUS - TSOLIDUS);
                    fraction[i][j][k] = ratio.powf(1.5);
                }
            }
        }
    }
    fraction
}

fn compute_temperature_gradient(temp_field: &[Vec<Vec<f64>>]) -> Vec<Vec<Vec<f64>>> {
    let n = temp_field.len();
    let mut grad = vec![vec![vec![0.0f64; n]; n]; n];
    let inv_2dx = 1.0 / (2.0 * CELL_SIZE);

    for i in 0..n {
        for j in 0..n {
            for k in 0..n {
                let dtdx = if i > 0 && i < n - 1 {
                    (temp_field[i + 1][j][k] - temp_field[i - 1][j][k]) * inv_2dx
                } else {
                    0.0
                };
                let dtdy = if j > 0 && j < n - 1 {
                    (temp_field[i][j + 1][k] - temp_field[i][j - 1][k]) * inv_2dx
                } else {
                    0.0
                };
                let dtdz = if k > 0 && k < n - 1 {
                    (temp_field[i][j][k + 1] - temp_field[i][j][k - 1]) * inv_2dx
                } else {
                    0.0
                };
                grad[i][j][k] = (dtdx * dtdx + dtdy * dtdy + dtdz * dtdz).sqrt();
            }
        }
    }
    grad
}

fn compute_cooling_rate_field(
    temp_t0: &[Vec<Vec<f64>>],
    temp_t1: &[Vec<Vec<f64>>],
) -> Vec<Vec<Vec<f64>>> {
    let n = temp_t0.len();
    let mut rate = vec![vec![vec![0.0f64; n]; n]; n];

    for i in 0..n {
        for j in 0..n {
            for k in 0..n {
                rate[i][j][k] = (temp_t0[i][j][k] - temp_t1[i][j][k]) / TIME_STEP;
            }
        }
    }
    rate
}

fn compute_niyama_criterion(
    temp_gradient: &[Vec<Vec<f64>>],
    cooling_rate: &[Vec<Vec<f64>>],
) -> Vec<Vec<Vec<f64>>> {
    let n = temp_gradient.len();
    let mut niyama = vec![vec![vec![0.0f64; n]; n]; n];
    let eps = 1e-6;

    for i in 0..n {
        for j in 0..n {
            for k in 0..n {
                let r = cooling_rate[i][j][k].abs().max(eps);
                niyama[i][j][k] = temp_gradient[i][j][k] / r.sqrt();
            }
        }
    }
    niyama
}

fn compute_shrinkage_porosity(
    solid_fraction: &[Vec<Vec<f64>>],
    temp_field: &[Vec<Vec<f64>>],
    niyama_field: &[Vec<Vec<f64>>],
) -> Vec<Vec<Vec<f64>>> {
    let n = solid_fraction.len();
    let mut porosity = vec![vec![vec![0.0f64; n]; n]; n];
    let mut rng = rand::thread_rng();

    for i in 0..n {
        for j in 0..n {
            for k in 0..n {
                let fs = solid_fraction[i][j][k];
                let niyama = niyama_field[i][j][k];

                if fs > 0.3 && fs < 0.95 {
                    let base_porosity = SHRINKAGE_COEFF * (1.0 - fs) * (fs - 0.3);

                    let niyama_factor = if niyama < NIYAMA_HIGH_RISK {
                        1.0 + 2.5 * (NIYAMA_HIGH_RISK - niyama) / NIYAMA_HIGH_RISK
                    } else if niyama < NIYAMA_CRITICAL {
                        1.0 + 1.2 * (NIYAMA_CRITICAL - niyama) / (NIYAMA_CRITICAL - NIYAMA_HIGH_RISK)
                    } else {
                        (NIYAMA_CRITICAL / niyama).powf(0.7)
                    };

                    let local_cooling = if i > 0 && i < n - 1 {
                        (temp_field[i + 1][j][k] - temp_field[i - 1][j][k]).abs()
                    } else {
                        0.0
                    };
                    let shape_factor = 1.0 + local_cooling / 150.0;

                    let noise = rng.gen_range(0.85..1.15);

                    porosity[i][j][k] = (base_porosity * niyama_factor * shape_factor * noise).max(0.0);
                } else {
                    porosity[i][j][k] = 0.0;
                }
            }
        }
    }

    apply_gaussian_blur(&mut porosity, 1);
    porosity
}

fn apply_gaussian_blur(field: &mut Vec<Vec<Vec<f64>>>, radius: usize) {
    let n = field.len();
    let mut temp = field.clone();
    let kernel = [
        [
            [1.0, 2.0, 1.0],
            [2.0, 4.0, 2.0],
            [1.0, 2.0, 1.0],
        ],
        [
            [2.0, 4.0, 2.0],
            [4.0, 8.0, 4.0],
            [2.0, 4.0, 2.0],
        ],
        [
            [1.0, 2.0, 1.0],
            [2.0, 4.0, 2.0],
            [1.0, 2.0, 1.0],
        ],
    ];
    let kernel_sum: f64 = kernel.iter().flatten().flatten().sum();

    for i in radius..n - radius {
        for j in radius..n - radius {
            for k in radius..n - radius {
                let mut sum = 0.0;
                for di in 0..=2 * radius {
                    for dj in 0..=2 * radius {
                        for dk in 0..=2 * radius {
                            sum += field[i + di - radius][j + dj - radius][k + dk - radius]
                                * kernel[di][dj][dk];
                        }
                    }
                }
                temp[i][j][k] = sum / kernel_sum;
            }
        }
    }
    *field = temp;
}

fn identify_defects(
    porosity: &[Vec<Vec<f64>>],
    threshold: f64,
) -> Vec<(f64, f64, f64, f64)> {
    let mut defects = Vec::new();
    let n = porosity.len();
    let mut visited = vec![vec![vec![false; n]; n]; n];

    for i in 0..n {
        for j in 0..n {
            for k in 0..n {
                if !visited[i][j][k] && porosity[i][j][k] > threshold {
                    let (cx, cy, cz, severity) =
                        flood_fill_defect(porosity, &mut visited, i, j, k, threshold);
                    defects.push((
                        cx as f64 / n as f64,
                        cy as f64 / n as f64,
                        cz as f64 / n as f64,
                        severity,
                    ));
                }
            }
        }
    }
    defects
}

fn flood_fill_defect(
    porosity: &[Vec<Vec<f64>>],
    visited: &mut Vec<Vec<Vec<bool>>>,
    si: usize,
    sj: usize,
    sk: usize,
    threshold: f64,
) -> (usize, usize, usize, f64) {
    let n = porosity.len();
    let mut stack = vec![(si, sj, sk)];
    let mut sum_x = 0usize;
    let mut sum_y = 0usize;
    let mut sum_z = 0usize;
    let mut count = 0usize;
    let mut max_p = 0.0f64;

    while let Some((i, j, k)) = stack.pop() {
        if i >= n || j >= n || k >= n || visited[i][j][k] || porosity[i][j][k] <= threshold {
            continue;
        }
        visited[i][j][k] = true;
        sum_x += i;
        sum_y += j;
        sum_z += k;
        count += 1;
        max_p = max_p.max(porosity[i][j][k]);

        if i > 0 { stack.push((i - 1, j, k)); }
        if i < n - 1 { stack.push((i + 1, j, k)); }
        if j > 0 { stack.push((i, j - 1, k)); }
        if j < n - 1 { stack.push((i, j + 1, k)); }
        if k > 0 { stack.push((i, j, k - 1)); }
        if k < n - 1 { stack.push((i, j, k + 1)); }
    }

    if count == 0 {
        (si, sj, sk, 0.0)
    } else {
        (sum_x / count, sum_y / count, sum_z / count, max_p)
    }
}
