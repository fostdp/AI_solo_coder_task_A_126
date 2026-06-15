use crate::alert_service::AlertService;
use crate::db::Database;
use crate::models::*;
use crate::simulation;
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use chrono::Utc;
use serde::Deserialize;
use std::sync::Arc;
use uuid::Uuid;

#[derive(Clone)]
pub struct AppState {
    pub db: Database,
    pub alert_service: Arc<AlertService>,
}

#[derive(Debug, Deserialize)]
pub struct Pagination {
    pub limit: Option<usize>,
}

pub async fn health_check() -> impl IntoResponse {
    Json(ApiResponse::ok(serde_json::json!({
        "status": "ok",
        "service": "bell-casting-backend",
        "version": "1.0.0",
        "timestamp": Utc::now().to_rfc3339(),
    })))
}

pub async fn get_bells(State(state): State<AppState>) -> impl IntoResponse {
    match state.db.get_all_bells().await {
        Ok(bells) => Json(ApiResponse::ok(bells)).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiResponse::<Vec<Bell>>::err(format!("{}", e))),
        )
            .into_response(),
    }
}

pub async fn get_bell(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> impl IntoResponse {
    match state.db.get_bell(id).await {
        Ok(Some(bell)) => Json(ApiResponse::ok(bell)).into_response(),
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(ApiResponse::<Bell>::err("钟体不存在")),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiResponse::<Bell>::err(format!("{}", e))),
        )
            .into_response(),
    }
}

pub async fn post_sensor_reading(
    State(state): State<AppState>,
    Json(input): Json<SensorReadingIn>,
) -> impl IntoResponse {
    let reading = SensorReading {
        reading_id: Uuid::new_v4(),
        bell_id: input.bell_id,
        timestamp: Utc::now(),
        temp_celsius: input.temp_celsius,
        temp_gradient: input.temp_gradient,
        wall_thickness_mm: input.wall_thickness_mm,
        thickness_deviation: input.thickness_deviation,
        alloy_cu: input.alloy_cu,
        alloy_sn: input.alloy_sn,
        alloy_pb: input.alloy_pb,
        alloy_zn: input.alloy_zn,
        alloy_other: input.alloy_other,
        acoustic_freq_hz: input.acoustic_freq_hz,
        acoustic_amplitude: input.acoustic_amplitude,
        acoustic_decay: input.acoustic_decay,
        acoustic_harmonics: input.acoustic_harmonics,
    };

    let bell_opt = state.db.get_bell(reading.bell_id).await.ok().flatten();

    let alerts = state
        .alert_service
        .check_sensor_reading(&reading, bell_opt.as_ref())
        .await
        .ok();

    match state.db.insert_sensor_reading(&reading).await {
        Ok(_) => Json(ApiResponse::ok(serde_json::json!({
            "reading_id": reading.reading_id,
            "alerts_triggered": alerts.map(|a| a.len()).unwrap_or(0),
        })))
        .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiResponse::<serde_json::Value>::err(format!("{}", e))),
        )
            .into_response(),
    }
}

pub async fn get_sensor_readings(
    State(state): State<AppState>,
    Path(bell_id): Path<Uuid>,
    Query(pagination): Query<Pagination>,
) -> impl IntoResponse {
    let limit = pagination.limit.unwrap_or(100);
    match state.db.get_sensor_readings(bell_id, limit).await {
        Ok(readings) => Json(ApiResponse::ok(readings)).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiResponse::<Vec<SensorReading>>::err(format!("{}", e))),
        )
            .into_response(),
    }
}

pub async fn run_casting_simulation(
    State(state): State<AppState>,
    Json(req): Json<CastingSimRequest>,
) -> impl IntoResponse {
    let sim = simulation::simulate_casting(&req);

    if let Err(e) = state
        .alert_service
        .check_casting_simulation(&sim)
        .await
    {
        tracing::warn!("Casting sim alert check error: {}", e);
    }

    match state.db.insert_casting_simulation(&sim).await {
        Ok(_) => Json(ApiResponse::ok(sim)).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiResponse::<CastingSimulation>::err(format!("{}", e))),
        )
            .into_response(),
    }
}

pub async fn get_casting_simulations(
    State(state): State<AppState>,
    Path(bell_id): Path<Uuid>,
    Query(pagination): Query<Pagination>,
) -> impl IntoResponse {
    let limit = pagination.limit.unwrap_or(20);
    match state.db.get_casting_simulations(bell_id, limit).await {
        Ok(sims) => Json(ApiResponse::ok(sims)).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiResponse::<Vec<CastingSimulation>>::err(format!("{}", e))),
        )
            .into_response(),
    }
}

pub async fn run_acoustic_simulation(
    State(state): State<AppState>,
    Json(req): Json<AcousticSimRequest>,
) -> impl IntoResponse {
    let bell_opt = state.db.get_bell(req.bell_id).await.ok().flatten();
    let sim = simulation::simulate_acoustic(&req, bell_opt.as_ref());

    if let Err(e) = state
        .alert_service
        .check_acoustic_simulation(&sim)
        .await
    {
        tracing::warn!("Acoustic sim alert check error: {}", e);
    }

    match state.db.insert_acoustic_simulation(&sim).await {
        Ok(_) => Json(ApiResponse::ok(sim)).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiResponse::<AcousticSimulation>::err(format!("{}", e))),
        )
            .into_response(),
    }
}

pub async fn get_acoustic_simulations(
    State(state): State<AppState>,
    Path(bell_id): Path<Uuid>,
    Query(pagination): Query<Pagination>,
) -> impl IntoResponse {
    let limit = pagination.limit.unwrap_or(20);
    match state.db.get_acoustic_simulations(bell_id, limit).await {
        Ok(sims) => Json(ApiResponse::ok(sims)).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiResponse::<Vec<AcousticSimulation>>::err(format!("{}", e))),
        )
            .into_response(),
    }
}

pub async fn get_active_alerts(State(state): State<AppState>) -> impl IntoResponse {
    match state.db.get_active_alerts().await {
        Ok(alerts) => Json(ApiResponse::ok(alerts)).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiResponse::<Vec<Alert>>::err(format!("{}", e))),
        )
            .into_response(),
    }
}

pub async fn resolve_alert(
    State(state): State<AppState>,
    Path(alert_id): Path<Uuid>,
) -> impl IntoResponse {
    match state.db.resolve_alert(alert_id).await {
        Ok(_) => Json(ApiResponse::ok(serde_json::json!({
            "alert_id": alert_id,
            "resolved": true,
        })))
        .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiResponse::<serde_json::Value>::err(format!("{}", e))),
        )
            .into_response(),
    }
}

pub async fn post_casting_process(
    State(state): State<AppState>,
    Json(input): Json<CastingProcess>,
) -> impl IntoResponse {
    let process = CastingProcess {
        process_id: Uuid::new_v4(),
        timestamp: Utc::now(),
        ..input
    };
    match state.db.insert_casting_process(&process).await {
        Ok(_) => Json(ApiResponse::ok(process)).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiResponse::<CastingProcess>::err(format!("{}", e))),
        )
            .into_response(),
    }
}

pub async fn get_casting_process(
    State(state): State<AppState>,
    Path(bell_id): Path<Uuid>,
    Query(pagination): Query<Pagination>,
) -> impl IntoResponse {
    let limit = pagination.limit.unwrap_or(50);
    match state.db.get_casting_process(bell_id, limit).await {
        Ok(processes) => Json(ApiResponse::ok(processes)).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiResponse::<Vec<CastingProcess>>::err(format!("{}", e))),
        )
            .into_response(),
    }
}
