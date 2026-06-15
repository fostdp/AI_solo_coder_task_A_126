use crate::config::Config;
use crate::models::MqttAlertPayload;
use anyhow::{Context, Result};
use rumqttc::{AsyncClient, MqttOptions, QoS};
use serde_json;
use std::time::Duration;
use tracing::{error, info};

#[derive(Clone)]
pub struct MqttNotifier {
    client: AsyncClient,
    topic: String,
}

impl MqttNotifier {
    pub async fn new(config: &Config) -> Result<Self> {
        let mut mqttoptions = MqttOptions::new(
            &config.mqtt_client_id,
            &config.mqtt_host,
            config.mqtt_port,
        );
        mqttoptions.set_keep_alive(Duration::from_secs(60));

        if let (Some(username), Some(password)) = (
            config.mqtt_username.as_ref(),
            config.mqtt_password.as_ref(),
        ) {
            mqttoptions.set_credentials(username, password);
        }

        let (client, mut eventloop) = AsyncClient::new(mqttoptions, 100);

        let client_clone = client.clone();
        tokio::spawn(async move {
            loop {
                match eventloop.poll().await {
                    Ok(_) => {}
                    Err(e) => {
                        error!("MQTT event loop error: {}", e);
                        tokio::time::sleep(Duration::from_secs(5)).await;
                    }
                }
            }
        });

        tokio::time::sleep(Duration::from_millis(500)).await;

        Ok(Self {
            client: client_clone,
            topic: config.mqtt_topic.clone(),
        })
    }

    pub async fn publish_alert(&self, payload: &MqttAlertPayload) -> Result<()> {
        let json = serde_json::to_string(payload).context("Failed to serialize alert")?;
        self.client
            .publish(&self.topic, QoS::AtLeastOnce, false, json.as_bytes())
            .await
            .context("Failed to publish MQTT alert")?;
        info!(
            "Published MQTT alert: bell={}, type={}, severity={}",
            payload.bell_name, payload.alert_type, payload.severity
        );
        Ok(())
    }
}
