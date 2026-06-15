use crate::config_loader::*;
use crate::db::Database;
use crate::message_bus::*;
use crate::models::*;
use anyhow::Result;
use chrono::Utc;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

pub struct DtuReceiver {
    db: Database,
    rx: SensorToDtuRx,
    tx_to_casting: DtuToCastingTx,
    tx_to_acoustic: DtuToAcousticTx,
    tx_to_alarm: DtuToAlarmTx,
}

impl DtuReceiver {
    pub fn new(
        db: Database,
        rx: SensorToDtuRx,
        tx_to_casting: DtuToCastingTx,
        tx_to_acoustic: DtuToAcousticTx,
        tx_to_alarm: DtuToAlarmTx,
    ) -> Self {
        Self {
            db,
            rx,
            tx_to_casting,
            tx_to_acoustic,
            tx_to_alarm,
        }
    }

    pub async fn run(mut self) {
        info!("DtuReceiver 模块启动");

        while let Some(msg) = self.rx.recv().await {
            match msg {
                BusMessage::SensorReadingReceived { reading, bell } => {
                    if let Err(e) = self.process_sensor_reading(reading, bell.as_ref()).await {
                        error!("处理传感器数据失败: {}", e);
                    }
                }
                BusMessage::Shutdown => {
                    info!("DtuReceiver 收到关闭信号，退出");
                    break;
                }
                _ => {}
            }
        }

        warn!("DtuReceiver 消息通道已关闭，模块退出");
    }

    pub fn validate_sensor_reading(
        &self,
        reading: &SensorReading,
        material: &Material,
    ) -> ValidationResult {
        let mut errors = Vec::new();

        if reading.temp_celsius < -50.0 || reading.temp_celsius > 1500.0 {
            errors.push(ValidationError {
                field: "temp_celsius".to_string(),
                message: "温度超出物理范围".to_string(),
                value: reading.temp_celsius,
            });
        }

        if reading.temp_gradient < 0.0 || reading.temp_gradient > 500.0 {
            errors.push(ValidationError {
                field: "temp_gradient".to_string(),
                message: "温度梯度超出范围".to_string(),
                value: reading.temp_gradient,
            });
        }

        if reading.wall_thickness_mm < 1.0 || reading.wall_thickness_mm > 500.0 {
            errors.push(ValidationError {
                field: "wall_thickness_mm".to_string(),
                message: "壁厚超出物理范围".to_string(),
                value: reading.wall_thickness_mm,
            });
        }

        if reading.thickness_deviation < -50.0 || reading.thickness_deviation > 50.0 {
            errors.push(ValidationError {
                field: "thickness_deviation".to_string(),
                message: "壁厚偏差超出范围".to_string(),
                value: reading.thickness_deviation,
            });
        }

        let alloy_sum = reading.alloy_cu
            + reading.alloy_sn
            + reading.alloy_pb
            + reading.alloy_zn
            + reading.alloy_other;
        let tolerance = ACOUSTIC_PARAMS.alert_thresholds.alloy_sum_tolerance_pct;
        if (alloy_sum - 100.0).abs() > tolerance {
            errors.push(ValidationError {
                field: "alloy_sum".to_string(),
                message: format!("合金成分总和偏离100%超过{}%", tolerance),
                value: alloy_sum,
            });
        }

        for (field, val) in [
            ("alloy_cu", reading.alloy_cu),
            ("alloy_sn", reading.alloy_sn),
            ("alloy_pb", reading.alloy_pb),
            ("alloy_zn", reading.alloy_zn),
            ("alloy_other", reading.alloy_other),
        ] {
            if val < 0.0 || val > 100.0 {
                errors.push(ValidationError {
                    field: field.to_string(),
                    message: "合金成分百分比超出范围".to_string(),
                    value: val,
                });
            }
        }

        if reading.acoustic_freq_hz < 10.0 || reading.acoustic_freq_hz > 20000.0 {
            errors.push(ValidationError {
                field: "acoustic_freq_hz".to_string(),
                message: "声学频率超出可闻范围".to_string(),
                value: reading.acoustic_freq_hz,
            });
        }

        if reading.acoustic_amplitude < 0.0 || reading.acoustic_amplitude > 10.0 {
            errors.push(ValidationError {
                field: "acoustic_amplitude".to_string(),
                message: "振幅超出范围".to_string(),
                value: reading.acoustic_amplitude,
            });
        }

        if reading.acoustic_decay < 0.0 || reading.acoustic_decay > 100.0 {
            errors.push(ValidationError {
                field: "acoustic_decay".to_string(),
                message: "衰减系数超出范围".to_string(),
                value: reading.acoustic_decay,
            });
        }

        let mut corrected = reading.clone();
        if (alloy_sum - 100.0).abs() <= tolerance * 2.0 && (alloy_sum - 100.0).abs() > tolerance {
            let factor = 100.0 / alloy_sum;
            corrected.alloy_cu *= factor;
            corrected.alloy_sn *= factor;
            corrected.alloy_pb *= factor;
            corrected.alloy_zn *= factor;
            corrected.alloy_other *= factor;
            warn!(
                "自动修正合金成分: 原总和{:.2}% -> 归一化后100%",
                alloy_sum
            );
        }

        ValidationResult {
            valid: errors.is_empty(),
            errors,
            corrected_reading: None,
        }
    }

    pub async fn process_sensor_reading(
        &self,
        mut reading: SensorReading,
        bell: Option<&Bell>,
    ) -> Result<SensorReading> {
        let material = bell
            .map(|b| get_material_for_bell(&b.bell_type))
            .unwrap_or_else(get_default_material);

        let validation = self.validate_sensor_reading(&reading, material);

        if !validation.valid {
            warn!(
                "传感器数据校验失败 ({} 个错误): {:?}",
                validation.errors.len(),
                validation.errors
            );
            for err in &validation.errors {
                error!("[校验错误] {}: {} (值={})", err.field, err.message, err.value);
            }
            return Err(anyhow::anyhow!(
                "Sensor reading validation failed: {} errors",
                validation.errors.len()
            ));
        }

        if let Some(corrected) = validation.corrected_reading {
            reading = corrected;
            info!("传感器数据已自动修正");
        }

        self.db.insert_sensor_reading(&reading).await?;
        debug!(
            "传感器数据已入库: bell={}, temp={:.1}°C, freq={:.2}Hz",
            reading.bell_id, reading.temp_celsius, reading.acoustic_freq_hz
        );

        let msg = BusMessage::SensorReadingReceived {
            reading: reading.clone(),
            bell: bell.cloned(),
        };

        if let Err(e) = self.tx_to_alarm.send(msg.clone()).await {
            error!("发送到告警模块失败: {}", e);
        }

        if reading.temp_celsius > 800.0 {
            if let Err(e) = self.tx_to_casting.send(msg.clone()).await {
                error!("发送到铸造仿真模块失败: {}", e);
            }
        }

        if reading.acoustic_freq_hz > 0.0 {
            if let Err(e) = self.tx_to_acoustic.send(msg).await {
                error!("发送到声学仿真模块失败: {}", e);
            }
        }

        Ok(reading)
    }
}
