use crate::config::Config;
use crate::models::*;
use anyhow::{Context, Result};
use chrono::Utc;
use clickhouse::{Client, Row};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone)]
pub struct Database {
    client: Client,
}

#[derive(Debug, Row, Serialize, Deserialize)]
pub struct BellRow {
    pub bell_id: Uuid,
    pub bell_name: String,
    pub dynasty: String,
    pub bell_type: String,
    pub material: String,
    pub height_m: f64,
    pub diameter_m: f64,
    pub weight_kg: f64,
    pub expected_pitch: String,
    pub expected_freq_hz: f64,
}

impl Database {
    pub fn new(config: &Config) -> Self {
        let client = Client::default()
            .with_url(&config.clickhouse_url)
            .with_user(&config.clickhouse_user)
            .with_password(&config.clickhouse_password)
            .with_database(&config.clickhouse_database);
        Self { client }
    }

    pub async fn ping(&self) -> Result<()> {
        let _: u64 = self
            .client
            .query("SELECT 1")
            .fetch_one()
            .await
            .context("ClickHouse ping failed")?;
        Ok(())
    }

    pub async fn get_all_bells(&self) -> Result<Vec<Bell>> {
        let bells: Vec<BellRow> = self
            .client
            .query(
                "SELECT bell_id, bell_name, dynasty, bell_type, material,
                 height_m, diameter_m, weight_kg, expected_pitch, expected_freq_hz
                 FROM bells ORDER BY created_at",
            )
            .fetch_all()
            .await
            .context("Failed to fetch bells")?;

        Ok(bells
            .into_iter()
            .map(|b| Bell {
                bell_id: b.bell_id,
                bell_name: b.bell_name,
                dynasty: b.dynasty,
                bell_type: b.bell_type,
                material: b.material,
                height_m: b.height_m,
                diameter_m: b.diameter_m,
                weight_kg: b.weight_kg,
                expected_pitch: b.expected_pitch,
                expected_freq_hz: b.expected_freq_hz,
                created_at: Utc::now(),
            })
            .collect())
    }

    pub async fn get_bell(&self, bell_id: Uuid) -> Result<Option<Bell>> {
        let bells = self.get_all_bells().await?;
        Ok(bells.into_iter().find(|b| b.bell_id == bell_id))
    }

    pub async fn insert_sensor_reading(&self, reading: &SensorReading) -> Result<()> {
        let mut insert = self.client.insert("sensor_readings")?;
        insert
            .write(reading)
            .await
            .context("Failed to insert sensor reading")?;
        insert.end().await.context("Failed to commit insert")?;
        Ok(())
    }

    pub async fn get_sensor_readings(&self, bell_id: Uuid, limit: usize) -> Result<Vec<SensorReading>> {
        let query = format!(
            "SELECT * FROM sensor_readings WHERE bell_id = toUUID('{}')
             ORDER BY timestamp DESC LIMIT {}",
            bell_id, limit
        );
        let readings: Vec<SensorReading> = self
            .client
            .query(&query)
            .fetch_all()
            .await
            .context("Failed to fetch sensor readings")?;
        Ok(readings)
    }

    pub async fn insert_casting_simulation(&self, sim: &CastingSimulation) -> Result<()> {
        let mut insert = self.client.insert("casting_simulation")?;
        insert
            .write(sim)
            .await
            .context("Failed to insert casting simulation")?;
        insert.end().await.context("Failed to commit insert")?;
        Ok(())
    }

    pub async fn get_casting_simulations(
        &self,
        bell_id: Uuid,
        limit: usize,
    ) -> Result<Vec<CastingSimulation>> {
        let query = format!(
            "SELECT * FROM casting_simulation WHERE bell_id = toUUID('{}')
             ORDER BY timestamp DESC LIMIT {}",
            bell_id, limit
        );
        let sims: Vec<CastingSimulation> = self
            .client
            .query(&query)
            .fetch_all()
            .await
            .context("Failed to fetch casting simulations")?;
        Ok(sims)
    }

    pub async fn insert_acoustic_simulation(&self, sim: &AcousticSimulation) -> Result<()> {
        let mut insert = self.client.insert("acoustic_simulation")?;
        insert
            .write(sim)
            .await
            .context("Failed to insert acoustic simulation")?;
        insert.end().await.context("Failed to commit insert")?;
        Ok(())
    }

    pub async fn get_acoustic_simulations(
        &self,
        bell_id: Uuid,
        limit: usize,
    ) -> Result<Vec<AcousticSimulation>> {
        let query = format!(
            "SELECT * FROM acoustic_simulation WHERE bell_id = toUUID('{}')
             ORDER BY timestamp DESC LIMIT {}",
            bell_id, limit
        );
        let sims: Vec<AcousticSimulation> = self
            .client
            .query(&query)
            .fetch_all()
            .await
            .context("Failed to fetch acoustic simulations")?;
        Ok(sims)
    }

    pub async fn insert_alert(&self, alert: &Alert) -> Result<()> {
        let mut insert = self.client.insert("alerts")?;
        insert
            .write(alert)
            .await
            .context("Failed to insert alert")?;
        insert.end().await.context("Failed to commit insert")?;
        Ok(())
    }

    pub async fn get_active_alerts(&self) -> Result<Vec<Alert>> {
        let alerts: Vec<Alert> = self
            .client
            .query(
                "SELECT * FROM alerts WHERE resolved = false ORDER BY timestamp DESC LIMIT 100",
            )
            .fetch_all()
            .await
            .context("Failed to fetch active alerts")?;
        Ok(alerts)
    }

    pub async fn resolve_alert(&self, alert_id: Uuid) -> Result<()> {
        let query = format!(
            "ALTER TABLE alerts UPDATE resolved = true, resolved_at = now()
             WHERE alert_id = toUUID('{}')",
            alert_id
        );
        self.client
            .query(&query)
            .execute()
            .await
            .context("Failed to resolve alert")?;
        Ok(())
    }

    pub async fn insert_casting_process(&self, process: &CastingProcess) -> Result<()> {
        let mut insert = self.client.insert("casting_process")?;
        insert
            .write(process)
            .await
            .context("Failed to insert casting process")?;
        insert.end().await.context("Failed to commit insert")?;
        Ok(())
    }

    pub async fn get_casting_process(&self, bell_id: Uuid, limit: usize) -> Result<Vec<CastingProcess>> {
        let query = format!(
            "SELECT * FROM casting_process WHERE bell_id = toUUID('{}')
             ORDER BY timestamp DESC LIMIT {}",
            bell_id, limit
        );
        let processes: Vec<CastingProcess> = self
            .client
            .query(&query)
            .fetch_all()
            .await
            .context("Failed to fetch casting processes")?;
        Ok(processes)
    }
}
