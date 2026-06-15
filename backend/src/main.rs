mod alert_service;
mod config;
mod db;
mod handlers;
mod models;
mod mqtt_client;
mod simulation;

use alert_service::AlertService;
use axum::{
    routing::{get, post},
    Router,
};
use clap::Parser;
use config::Config;
use db::Database;
use handlers::AppState;
use mqtt_client::MqttNotifier;
use std::net::SocketAddr;
use std::sync::Arc;
use tower_http::cors::{Any, CorsLayer};
use tracing::{error, info, warn};
use tracing_subscriber::EnvFilter;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    #[arg(long, default_value_t = false)]
    skip_mqtt: bool,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    let args = Args::parse();
    let config = Config::from_env();

    info!("Starting bell-casting backend on port {}", config.server_port);

    let db = Database::new(&config);
    match db.ping().await {
        Ok(_) => info!("Connected to ClickHouse successfully"),
        Err(e) => warn!("Failed to connect to ClickHouse: {} (continuing anyway)", e),
    }

    let mqtt_notifier = if args.skip_mqtt {
        info!("MQTT disabled via --skip-mqtt");
        None
    } else {
        match MqttNotifier::new(&config).await {
            Ok(mqtt) => {
                info!("MQTT client initialized: {}:{}", config.mqtt_host, config.mqtt_port);
                Some(mqtt)
            }
            Err(e) => {
                warn!("Failed to initialize MQTT (continuing without alerts): {}", e);
                None
            }
        }
    };

    let alert_service = Arc::new(AlertService::new(db.clone(), mqtt_notifier));
    let app_state = AppState {
        db: db.clone(),
        alert_service,
    };

    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    let app = Router::new()
        .route("/health", get(handlers::health_check))
        .route("/bells", get(handlers::get_bells))
        .route("/bells/:id", get(handlers::get_bell))
        .route("/sensors", post(handlers::post_sensor_reading))
        .route("/sensors/bell/:bell_id", get(handlers::get_sensor_readings))
        .route("/sim/casting", post(handlers::run_casting_simulation))
        .route("/sim/casting/bell/:bell_id", get(handlers::get_casting_simulations))
        .route("/sim/acoustic", post(handlers::run_acoustic_simulation))
        .route("/sim/acoustic/bell/:bell_id", get(handlers::get_acoustic_simulations))
        .route("/alerts", get(handlers::get_active_alerts))
        .route("/alerts/:id/resolve", post(handlers::resolve_alert))
        .route("/casting-process", post(handlers::post_casting_process))
        .route("/casting-process/bell/:bell_id", get(handlers::get_casting_process))
        .with_state(app_state)
        .layer(cors);

    let addr: SocketAddr = format!("0.0.0.0:{}", config.server_port).parse()?;
    info!("Listening on http://{}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app.into_make_service())
        .await
        .map_err(|e| {
            error!("Server error: {}", e);
            anyhow::anyhow!(e)
        })?;

    Ok(())
}
