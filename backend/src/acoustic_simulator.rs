use crate::config_loader::{get_material_for_bell, get_default_material};
use crate::db::Database;
use crate::message_bus::*;
use crate::models::*;
use crate::simulation::acoustic::simulate_acoustic;
use anyhow::Result;
use tracing::{debug, error, info, warn};

pub struct AcousticSimulator {
    db: Database,
    rx_from_dtu: DtuToAcousticRx,
    rx_external: SensorToAcousticRx,
    tx_to_alarm: AcousticToAlarmTx,
}

impl AcousticSimulator {
    pub fn new(
        db: Database,
        rx_from_dtu: DtuToAcousticRx,
        rx_external: SensorToAcousticRx,
        tx_to_alarm: AcousticToAlarmTx,
    ) -> Self {
        Self {
            db,
            rx_from_dtu,
            rx_external,
            tx_to_alarm,
        }
    }

    pub async fn run(mut self) {
        info!("AcousticSimulator 模块启动");

        loop {
            tokio::select! {
                Some(msg) = self.rx_from_dtu.recv() => {
                    if let BusMessage::SensorReadingReceived { reading, bell } = msg {
                        if reading.acoustic_freq_hz > 0.0 && reading.temp_celsius < 100.0 {
                            if let Err(e) = self.process_sensor_reading(&reading, bell.as_ref()).await {
                                error!("处理声学传感器数据失败: {}", e);
                            }
                        }
                    }
                }
                Some(msg) = self.rx_external.recv() => {
                    match msg {
                        BusMessage::AcousticSimRequested { req, bell } => {
                            if let Err(e) = self.process_sim_request(&req, bell.as_ref()).await {
                                error!("处理声学仿真请求失败: {}", e);
                            }
                        }
                        BusMessage::Shutdown => {
                            info!("AcousticSimulator 收到关闭信号，退出");
                            break;
                        }
                        _ => {}
                    }
                }
                else => {
                    warn!("AcousticSimulator 所有消息通道已关闭，模块退出");
                    break;
                }
            }
        }
    }

    async fn process_sensor_reading(
        &self,
        reading: &SensorReading,
        bell: Option<&Bell>,
    ) -> Result<()> {
        let sensor_freq = reading.acoustic_freq_hz;
        let expected_freq = bell.map(|b| b.expected_freq_hz).unwrap_or(261.63);
        let deviation = 1200.0 * (sensor_freq / expected_freq).log2().abs();

        if deviation < 30.0 {
            return Ok(());
        }

        debug!(
            "音准偏差触发声学仿真: bell={}, freq={:.2}Hz, deviation={:.1}音分",
            reading.bell_id, sensor_freq, deviation
        );

        let material = bell
            .map(|b| get_material_for_bell(&b.bell_type))
            .unwrap_or_else(get_default_material);

        let req = AcousticSimRequest {
            bell_id: reading.bell_id,
            method: "auto_from_sensor".to_string(),
            young_modulus: Some(material.young_modulus),
            poisson_ratio: Some(material.poisson_ratio),
            density: Some(material.density),
        };

        let sim = simulate_acoustic(&req, bell, material);
        self.db.insert_acoustic_simulation(&sim).await?;

        info!(
            "自动声学仿真完成: bell={}, pitch_ok={}, deviation={:.1}音分",
            sim.bell_id, sim.pitch_ok, sim.pitch_deviation_cents
        );

        let msg = BusMessage::AcousticSimCompleted { sim };
        if let Err(e) = self.tx_to_alarm.send(msg).await {
            error!("发送声学仿真结果到告警模块失败: {}", e);
        }

        Ok(())
    }

    async fn process_sim_request(
        &self,
        req: &AcousticSimRequest,
        bell: Option<&Bell>,
    ) -> Result<AcousticSimulation> {
        info!(
            "执行声学仿真: bell={}, method={}",
            req.bell_id, req.method
        );

        let material = bell
            .map(|b| get_material_for_bell(&b.bell_type))
            .unwrap_or_else(get_default_material);

        debug!("使用材料参数: {}", material.name);

        let sim = tokio::task::spawn_blocking({
            let req = req.clone();
            let bell = bell.cloned();
            let material = material.clone();
            move || simulate_acoustic(&req, bell.as_ref(), &material)
        })
        .await
        .map_err(|e| anyhow::anyhow!("仿真任务被中止: {}", e))?;

        self.db.insert_acoustic_simulation(&sim).await?;

        info!(
            "声学仿真完成: bell={}, pitch_ok={}, deviation={:.1}音分, power={:.4}W",
            sim.bell_id, sim.pitch_ok, sim.pitch_deviation_cents, sim.sound_power
        );

        let msg = BusMessage::AcousticSimCompleted { sim: sim.clone() };
        if let Err(e) = self.tx_to_alarm.send(msg).await {
            error!("发送声学仿真结果到告警模块失败: {}", e);
        }

        Ok(sim)
    }
}
