use crate::config_loader::{get_material_for_bell, get_default_material};
use crate::db::Database;
use crate::message_bus::*;
use crate::models::*;
use crate::simulation::casting::simulate_casting;
use anyhow::Result;
use tracing::{debug, error, info, warn};

pub struct CastingSimulator {
    db: Database,
    rx_from_dtu: DtuToCastingRx,
    rx_external: SensorToCastingRx,
    tx_to_alarm: CastingToAlarmTx,
}

impl CastingSimulator {
    pub fn new(
        db: Database,
        rx_from_dtu: DtuToCastingRx,
        rx_external: SensorToCastingRx,
        tx_to_alarm: CastingToAlarmTx,
    ) -> Self {
        Self {
            db,
            rx_from_dtu,
            rx_external,
            tx_to_alarm,
        }
    }

    pub async fn run(mut self) {
        info!("CastingSimulator 模块启动");

        loop {
            tokio::select! {
                Some(msg) = self.rx_from_dtu.recv() => {
                    if let BusMessage::SensorReadingReceived { reading, bell } = msg {
                        if let Err(e) = self.process_sensor_reading(&reading, bell.as_ref()).await {
                            error!("处理传感器数据失败: {}", e);
                        }
                    }
                }
                Some(msg) = self.rx_external.recv() => {
                    match msg {
                        BusMessage::CastingSimRequested { req, bell } => {
                            if let Err(e) = self.process_sim_request(&req, bell.as_ref()).await {
                                error!("处理铸造仿真请求失败: {}", e);
                            }
                        }
                        BusMessage::Shutdown => {
                            info!("CastingSimulator 收到关闭信号，退出");
                            break;
                        }
                        _ => {}
                    }
                }
                else => {
                    warn!("CastingSimulator 所有消息通道已关闭，模块退出");
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
        if reading.temp_celsius <= 800.0 {
            return Ok(());
        }

        debug!(
            "温度触发铸造仿真: bell={}, temp={:.1}°C",
            reading.bell_id, reading.temp_celsius
        );

        let material = bell
            .map(|b| get_material_for_bell(&b.bell_type))
            .unwrap_or_else(get_default_material);

        let req = CastingSimRequest {
            bell_id: reading.bell_id,
            sim_type: "auto_from_sensor".to_string(),
            initial_temp: reading.temp_celsius,
            grid_size: Some(16),
        };

        let sim = simulate_casting(&req, material);
        self.db.insert_casting_simulation(&sim).await?;

        info!(
            "自动铸造仿真完成: bell={}, risk={}, defects={}, shrinkage={:.2}%",
            sim.bell_id,
            sim.prediction_risk,
            sim.defect_count,
            sim.max_shrinkage * 100.0
        );

        let msg = BusMessage::CastingSimCompleted { sim };
        if let Err(e) = self.tx_to_alarm.send(msg).await {
            error!("发送铸造仿真结果到告警模块失败: {}", e);
        }

        Ok(())
    }

    async fn process_sim_request(
        &self,
        req: &CastingSimRequest,
        bell: Option<&Bell>,
    ) -> Result<CastingSimulation> {
        info!(
            "执行铸造仿真: bell={}, type={}, init_temp={:.1}°C",
            req.bell_id, req.sim_type, req.initial_temp
        );

        let material = bell
            .map(|b| get_material_for_bell(&b.bell_type))
            .unwrap_or_else(get_default_material);

        debug!("使用材料参数: {}", material.name);

        let sim = tokio::task::spawn_blocking({
            let req = req.clone();
            let material = material.clone();
            move || simulate_casting(&req, &material)
        })
        .await
        .map_err(|e| anyhow::anyhow!("仿真任务被中止: {}", e))?;

        self.db.insert_casting_simulation(&sim).await?;

        info!(
            "铸造仿真完成: bell={}, risk={}, defects={}, shrinkage={:.2}%",
            sim.bell_id,
            sim.prediction_risk,
            sim.defect_count,
            sim.max_shrinkage * 100.0
        );

        let msg = BusMessage::CastingSimCompleted { sim: sim.clone() };
        if let Err(e) = self.tx_to_alarm.send(msg).await {
            error!("发送铸造仿真结果到告警模块失败: {}", e);
        }

        Ok(sim)
    }
}
