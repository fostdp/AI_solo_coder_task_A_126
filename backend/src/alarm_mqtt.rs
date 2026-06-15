use crate::config_loader::ACOUSTIC_PARAMS;
use crate::db::Database;
use crate::message_bus::*;
use crate::models::*;
use crate::mqtt_client::MqttNotifier;
use anyhow::Result;
use chrono::Utc;
use tracing::{error, info, warn};
use uuid::Uuid;

pub struct AlarmMqttService {
    db: Database,
    rx_sensor: DtuToAlarmRx,
    rx_casting: CastingToAlarmRx,
    rx_acoustic: AcousticToAlarmRx,
    mqtt: Option<MqttNotifier>,
}

impl AlarmMqttService {
    pub fn new(
        db: Database,
        rx_sensor: DtuToAlarmRx,
        rx_casting: CastingToAlarmRx,
        rx_acoustic: AcousticToAlarmRx,
        mqtt: Option<MqttNotifier>,
    ) -> Self {
        Self {
            db,
            rx_sensor,
            rx_casting,
            rx_acoustic,
            mqtt,
        }
    }

    pub async fn run(mut self) {
        info!("AlarmMqttService 模块启动");

        loop {
            tokio::select! {
                Some(msg) = self.rx_sensor.recv() => {
                    if let BusMessage::SensorReadingReceived { reading, bell } = msg {
                        if let Err(e) = self.check_sensor_reading(&reading, bell.as_ref()).await {
                            error!("处理传感器告警失败: {}", e);
                        }
                    }
                }
                Some(msg) = self.rx_casting.recv() => {
                    match msg {
                        BusMessage::CastingSimCompleted { sim } => {
                            if let Err(e) = self.check_casting_simulation(&sim).await {
                                error!("处理铸造仿真告警失败: {}", e);
                            }
                        }
                        BusMessage::Shutdown => {
                            info!("AlarmMqttService 收到关闭信号，退出");
                            break;
                        }
                        _ => {}
                    }
                }
                Some(msg) = self.rx_acoustic.recv() => {
                    match msg {
                        BusMessage::AcousticSimCompleted { sim } => {
                            if let Err(e) = self.check_acoustic_simulation(&sim).await {
                                error!("处理声学仿真告警失败: {}", e);
                            }
                        }
                        BusMessage::Shutdown => {
                            info!("AlarmMqttService 收到关闭信号，退出");
                            break;
                        }
                        _ => {}
                    }
                }
                else => {
                    warn!("所有告警通道已关闭，退出");
                    break;
                }
            }
        }

        warn!("AlarmMqttService 模块退出");
    }

    async fn check_sensor_reading(
        &self,
        reading: &SensorReading,
        bell: Option<&Bell>,
    ) -> Result<Vec<Alert>> {
        let mut alerts = Vec::new();
        let thresholds = &ACOUSTIC_PARAMS.alert_thresholds;
        let pitch_tol = ACOUSTIC_PARAMS.bell_acoustics.pitch_tolerance_cents;
        let pitch_warn = ACOUSTIC_PARAMS.bell_acoustics.pitch_warning_cents;
        let pitch_crit = ACOUSTIC_PARAMS.bell_acoustics.pitch_critical_cents;

        if let Some(bell) = bell {
            let pitch_cents = 1200.0 * (reading.acoustic_freq_hz / bell.expected_freq_hz).log2();
            if pitch_cents.abs() > pitch_tol {
                let severity = if pitch_cents.abs() > pitch_crit {
                    "critical".to_string()
                } else if pitch_cents.abs() > pitch_warn {
                    "danger".to_string()
                } else {
                    "warning".to_string()
                };
                let message = format!(
                    "音准偏差: {:.1}音分 (期望{:.2}Hz, 实测{:.2}Hz)",
                    pitch_cents, bell.expected_freq_hz, reading.acoustic_freq_hz
                );
                alerts.push(
                    self.create_alert(
                        reading.bell_id,
                        "pitch".to_string(),
                        severity,
                        message,
                        Some(reading.reading_id),
                        None,
                    )
                    .await?,
                );
            }
        }

        if reading.temp_celsius > thresholds.temperature_max_celsius {
            alerts.push(
                self.create_alert(
                    reading.bell_id,
                    "temp".to_string(),
                    "danger".to_string(),
                    format!(
                        "温度异常: {:.1}°C 超过安全阈值 {}°C",
                        reading.temp_celsius, thresholds.temperature_max_celsius
                    ),
                    Some(reading.reading_id),
                    None,
                )
                .await?,
            );
        }

        if reading.thickness_deviation.abs() > thresholds.thickness_deviation_warning_pct {
            let severity = if reading.thickness_deviation.abs()
                > thresholds.thickness_deviation_danger_pct
            {
                "danger"
            } else {
                "warning"
            }
            .to_string();
            alerts.push(
                self.create_alert(
                    reading.bell_id,
                    "defect".to_string(),
                    severity,
                    format!("壁厚偏差过大: {:.1}%", reading.thickness_deviation),
                    Some(reading.reading_id),
                    None,
                )
                .await?,
            );
        }

        let alloy_total = reading.alloy_cu
            + reading.alloy_sn
            + reading.alloy_pb
            + reading.alloy_zn
            + reading.alloy_other;
        if (alloy_total - 100.0).abs() > thresholds.alloy_sum_tolerance_pct {
            alerts.push(
                self.create_alert(
                    reading.bell_id,
                    "alloy".to_string(),
                    "warning".to_string(),
                    format!(
                        "合金成分偏差: 总和{:.2}%偏离100%超过{}%",
                        alloy_total, thresholds.alloy_sum_tolerance_pct
                    ),
                    Some(reading.reading_id),
                    None,
                )
                .await?,
            );
        }

        Ok(alerts)
    }

    async fn check_casting_simulation(
        &self,
        sim: &CastingSimulation,
    ) -> Result<Option<Alert>> {
        match sim.prediction_risk.as_str() {
            "critical" | "high" => {
                let severity = if sim.prediction_risk == "critical" {
                    "critical".to_string()
                } else {
                    "danger".to_string()
                };
                let message = format!(
                    "铸造缺陷风险: {} (缩孔率{:.2}%, 缺陷{}处)",
                    sim.prediction_risk,
                    sim.max_shrinkage * 100.0,
                    sim.defect_count
                );
                Ok(Some(
                    self.create_alert(
                        sim.bell_id,
                        "defect".to_string(),
                        severity,
                        message,
                        None,
                        Some(sim.sim_id),
                    )
                    .await?,
                ))
            }
            "medium" => {
                let message = format!(
                    "铸造缺陷预警: medium (缩孔率{:.2}%, 缺陷{}处)",
                    sim.max_shrinkage * 100.0,
                    sim.defect_count
                );
                Ok(Some(
                    self.create_alert(
                        sim.bell_id,
                        "defect".to_string(),
                        "warning".to_string(),
                        message,
                        None,
                        Some(sim.sim_id),
                    )
                    .await?,
                ))
            }
            _ => Ok(None),
        }
    }

    async fn check_acoustic_simulation(
        &self,
        sim: &AcousticSimulation,
    ) -> Result<Option<Alert>> {
        if !sim.pitch_ok {
            let pitch_crit = ACOUSTIC_PARAMS.bell_acoustics.pitch_critical_cents;
            let pitch_warn = ACOUSTIC_PARAMS.bell_acoustics.pitch_warning_cents;

            let severity = if sim.pitch_deviation_cents.abs() > pitch_crit {
                "critical".to_string()
            } else if sim.pitch_deviation_cents.abs() > pitch_warn {
                "danger".to_string()
            } else {
                "warning".to_string()
            };
            let message = format!(
                "声学仿真音准偏差: {:.1}音分",
                sim.pitch_deviation_cents
            );
            Ok(Some(
                self.create_alert(
                    sim.bell_id,
                    "pitch".to_string(),
                    severity,
                    message,
                    None,
                    Some(sim.sim_id),
                )
                .await?,
            ))
        } else {
            Ok(None)
        }
    }

    async fn create_alert(
        &self,
        bell_id: Uuid,
        alert_type: String,
        severity: String,
        message: String,
        related_reading: Option<Uuid>,
        related_sim: Option<Uuid>,
    ) -> Result<Alert> {
        let alert = Alert {
            alert_id: Uuid::new_v4(),
            bell_id,
            timestamp: Utc::now(),
            alert_type: alert_type.clone(),
            severity: severity.clone(),
            message: message.clone(),
            related_reading,
            related_sim,
            resolved: false,
            resolved_at: None,
        };

        self.db.insert_alert(&alert).await?;
        warn!(
            "告警生成: type={}, severity={}, bell={}, msg={}",
            alert_type, severity, bell_id, message
        );

        if let Some(mqtt) = &self.mqtt {
            let bell_name = self
                .db
                .get_bell(bell_id)
                .await
                .ok()
                .flatten()
                .map(|b| b.bell_name)
                .unwrap_or_else(|| "未知".to_string());

            let payload = MqttAlertPayload {
                alert_id: alert.alert_id,
                bell_id: alert.bell_id,
                bell_name,
                timestamp: alert.timestamp,
                alert_type: alert.alert_type.clone(),
                severity: alert.severity.clone(),
                message: alert.message.clone(),
            };

            if let Err(e) = mqtt.publish_alert(&payload).await {
                warn!("MQTT告警推送失败: {}", e);
            } else {
                info!("MQTT告警推送成功: id={}", alert.alert_id);
            }
        }

        Ok(alert)
    }
}
