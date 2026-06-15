mod alarm_mqtt;
mod alert_service;
mod acoustic_simulator;
mod casting_simulator;
mod config;
mod config_loader;
mod db;
mod dtu_receiver;
mod handlers;
mod message_bus;
mod models;
mod mqtt_client;
mod simulation;

use acoustic_simulator::AcousticSimulator;
use alarm_mqtt::AlarmMqttService;
use axum::{
    routing::{get, post},
    Router,
};
use casting_simulator::CastingSimulator;
use clap::Parser;
use config::Config;
use config_loader::{MATERIALS, ACOUSTIC_PARAMS};
use db::Database;
use dtu_receiver::DtuReceiver;
use handlers::AppState;
use message_bus::*;
use mqtt_client::MqttNotifier;
use std::net::SocketAddr;
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

    info!(
        "=== 古代铸钟工艺仿真与钟声传播模拟系统 (微服务架构) ==="
    );
    info!("加载材料配置: {} 种材料", MATERIALS.len());
    info!(
        "加载声学配置: BEM网格={}, 低频阈值={}Hz",
        ACOUSTIC_PARAMS.bem_solver.default_panels,
        ACOUSTIC_PARAMS.bem_solver.low_frequency_threshold_hz
    );
    info!("启动服务端口: {}", config.server_port);

    let db = Database::new(&config);
    match db.ping().await {
        Ok(_) => info!("✅ ClickHouse 连接成功"),
        Err(e) => warn!(
            "⚠️  ClickHouse 连接失败 (继续运行): {}",
            e
        ),
    }

    let mqtt_notifier = if args.skip_mqtt {
        info!("MQTT 已通过 --skip-mqtt 禁用");
        None
    } else {
        match MqttNotifier::new(&config).await {
            Ok(mqtt) => {
                info!(
                    "✅ MQTT 客户端初始化: {}:{}",
                    config.mqtt_host, config.mqtt_port
                );
                Some(mqtt)
            }
            Err(e) => {
                warn!("⚠️  MQTT 初始化失败 (无告警推送): {}", e);
                None
            }
        }
    };

    info!("--- 构建 tokio mpsc 通道 ---");
    const CHANNEL_CAPACITY: usize = 1000;

    let (tx_dtu, rx_dtu) = tokio::sync::mpsc::channel::<BusMessage>(CHANNEL_CAPACITY);
    let (tx_to_casting, rx_from_dtu_to_casting) =
        tokio::sync::mpsc::channel::<BusMessage>(CHANNEL_CAPACITY);
    let (tx_to_acoustic, rx_from_dtu_to_acoustic) =
        tokio::sync::mpsc::channel::<BusMessage>(CHANNEL_CAPACITY);
    let (tx_to_alarm_from_dtu, rx_from_dtu_to_alarm) =
        tokio::sync::mpsc::channel::<BusMessage>(CHANNEL_CAPACITY);
    let (tx_to_alarm_from_casting, rx_from_casting_to_alarm) =
        tokio::sync::mpsc::channel::<BusMessage>(CHANNEL_CAPACITY);
    let (tx_to_alarm_from_acoustic, rx_from_acoustic_to_alarm) =
        tokio::sync::mpsc::channel::<BusMessage>(CHANNEL_CAPACITY);
    let (tx_to_casting_external, rx_casting_external) =
        tokio::sync::mpsc::channel::<BusMessage>(CHANNEL_CAPACITY);
    let (tx_to_acoustic_external, rx_acoustic_external) =
        tokio::sync::mpsc::channel::<BusMessage>(CHANNEL_CAPACITY);

    info!("--- 启动4个微服务模块 ---");

    let dtu_receiver = DtuReceiver::new(
        db.clone(),
        rx_dtu,
        tx_to_casting.clone(),
        tx_to_acoustic.clone(),
        tx_to_alarm_from_dtu.clone(),
    );
    info!("✅ dtu_receiver 模块已初始化");

    let casting_simulator = CastingSimulator::new(
        db.clone(),
        rx_from_dtu_to_casting,
        rx_casting_external,
        tx_to_alarm_from_casting.clone(),
    );
    info!("✅ casting_simulator 模块已初始化");

    let acoustic_simulator = AcousticSimulator::new(
        db.clone(),
        rx_from_dtu_to_acoustic,
        rx_acoustic_external,
        tx_to_alarm_from_acoustic.clone(),
    );
    info!("✅ acoustic_simulator 模块已初始化");

    let alarm_mqtt = AlarmMqttService::new(
        db.clone(),
        rx_from_dtu_to_alarm,
        rx_from_casting_to_alarm,
        rx_from_acoustic_to_alarm,
        mqtt_notifier,
    );
    info!("✅ alarm_mqtt 模块已初始化");

    tokio::spawn(async move { dtu_receiver.run().await });
    tokio::spawn(async move { casting_simulator.run().await });
    tokio::spawn(async move { acoustic_simulator.run().await });
    tokio::spawn(async move { alarm_mqtt.run().await });

    info!("--- 启动 HTTP API 服务 ---");

    let app_state = AppState {
        db: db.clone(),
        tx_to_dtu: tx_dtu,
        tx_to_casting: tx_to_casting_external,
        tx_to_acoustic: tx_to_acoustic_external,
    };

    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    let app = axum::Router::new()
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
    info!("🚀 HTTP 服务监听 http://{}", addr);
    info!("=== 系统启动完成 ===");

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app.into_make_service())
        .await
        .map_err(|e| {
            error!("❌ 服务错误: {}", e);
            anyhow::anyhow!(e)
        })?;

    Ok(())
}
