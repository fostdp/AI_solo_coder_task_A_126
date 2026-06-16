use metrics::{
    counter, gauge, histogram, describe_counter, describe_gauge, describe_histogram,
    Unit,
};
use metrics_exporter_prometheus::PrometheusBuilder;
use std::net::SocketAddr;
use tokio::net::TcpListener;
use tracing::{error, info};

pub fn init_metrics() {
    describe_counter!("bell_sensor_readings_total", Unit::Count, "Total sensor readings received");
    describe_counter!("bell_sensor_readings_valid", Unit::Count, "Valid sensor readings");
    describe_counter!("bell_sensor_readings_invalid", Unit::Count, "Invalid sensor readings");
    describe_counter!("bell_casting_sim_total", Unit::Count, "Total casting simulations run");
    describe_counter!("bell_acoustic_sim_total", Unit::Count, "Total acoustic simulations run");
    describe_counter!("bell_alerts_total", Unit::Count, "Total alerts generated");
    describe_counter!("bell_mqtt_published_total", Unit::Count, "Total MQTT messages published");
    describe_counter!("bell_mqtt_failed_total", Unit::Count, "Total MQTT publish failures");

    describe_gauge!("bell_active_bells", Unit::Count, "Number of active bells");
    describe_gauge!("bell_active_alerts", Unit::Count, "Number of active alerts");

    describe_histogram!("bell_casting_sim_duration_seconds", Unit::Seconds, "Casting simulation duration");
    describe_histogram!("bell_acoustic_sim_duration_seconds", Unit::Seconds, "Acoustic simulation duration");
    describe_histogram!("bell_request_duration_seconds", Unit::Seconds, "HTTP request duration");
}

pub async fn start_metrics_server(bind_addr: &str) -> anyhow::Result<()> {
    let addr: SocketAddr = bind_addr.parse()?;

    let builder = PrometheusBuilder::new()
        .with_http_listener(addr)
        .install()
        .map_err(|e| anyhow::anyhow!("Failed to install Prometheus exporter: {}", e))?;

    info!("Prometheus metrics server listening on http://{}", addr);
    Ok(())
}

pub struct MetricsGuard;

impl MetricsGuard {
    pub fn new() -> Self {
        init_metrics();
        Self
    }
}

impl Default for MetricsGuard {
    fn default() -> Self {
        Self::new()
    }
}

pub fn increment_sensor_reading(valid: bool) {
    counter!("bell_sensor_readings_total").increment(1);
    if valid {
        counter!("bell_sensor_readings_valid").increment(1);
    } else {
        counter!("bell_sensor_readings_invalid").increment(1);
    }
}

pub fn increment_casting_sim() {
    counter!("bell_casting_sim_total").increment(1);
}

pub fn increment_acoustic_sim() {
    counter!("bell_acoustic_sim_total").increment(1);
}

pub fn increment_alert(alert_type: &str, severity: &str) {
    counter!("bell_alerts_total", "type" => alert_type.to_string(), "severity" => severity.to_string())
        .increment(1);
}

pub fn increment_mqtt_published(success: bool) {
    if success {
        counter!("bell_mqtt_published_total").increment(1);
    } else {
        counter!("bell_mqtt_failed_total").increment(1);
    }
}

pub fn set_active_bells(count: f64) {
    gauge!("bell_active_bells").set(count);
}

pub fn set_active_alerts(count: f64) {
    gauge!("bell_active_alerts").set(count);
}

pub fn record_casting_sim_duration(secs: f64) {
    histogram!("bell_casting_sim_duration_seconds").record(secs);
}

pub fn record_acoustic_sim_duration(secs: f64) {
    histogram!("bell_acoustic_sim_duration_seconds").record(secs);
}

pub fn record_request_duration(secs: f64, endpoint: &str, method: &str) {
    histogram!("bell_request_duration_seconds",
        "endpoint" => endpoint.to_string(),
        "method" => method.to_string()
    ).record(secs);
}
