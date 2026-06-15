use crate::models::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum BusMessage {
    SensorReadingReceived {
        reading: SensorReading,
        bell: Option<Bell>,
    },
    CastingSimRequested {
        req: CastingSimRequest,
        bell: Option<Bell>,
    },
    AcousticSimRequested {
        req: AcousticSimRequest,
        bell: Option<Bell>,
    },
    CastingSimCompleted {
        sim: CastingSimulation,
    },
    AcousticSimCompleted {
        sim: AcousticSimulation,
    },
    AlertGenerated {
        alert: Alert,
        bell_name: String,
    },
    Shutdown,
}

pub type SensorToDtuTx = tokio::sync::mpsc::Sender<BusMessage>;
pub type SensorToDtuRx = tokio::sync::mpsc::Receiver<BusMessage>;

pub type SensorToAlarmTx = tokio::sync::mpsc::Sender<BusMessage>;
pub type SensorToAlarmRx = tokio::sync::mpsc::Receiver<BusMessage>;

pub type SensorToCastingTx = tokio::sync::mpsc::Sender<BusMessage>;
pub type SensorToCastingRx = tokio::sync::mpsc::Receiver<BusMessage>;

pub type SensorToAcousticTx = tokio::sync::mpsc::Sender<BusMessage>;
pub type SensorToAcousticRx = tokio::sync::mpsc::Receiver<BusMessage>;

pub type CastingToAlarmTx = tokio::sync::mpsc::Sender<BusMessage>;
pub type CastingToAlarmRx = tokio::sync::mpsc::Receiver<BusMessage>;

pub type AcousticToAlarmTx = tokio::sync::mpsc::Sender<BusMessage>;
pub type AcousticToAlarmRx = tokio::sync::mpsc::Receiver<BusMessage>;

pub type DtuToCastingTx = tokio::sync::mpsc::Sender<BusMessage>;
pub type DtuToCastingRx = tokio::sync::mpsc::Receiver<BusMessage>;

pub type DtuToAcousticTx = tokio::sync::mpsc::Sender<BusMessage>;
pub type DtuToAcousticRx = tokio::sync::mpsc::Receiver<BusMessage>;

pub type DtuToAlarmTx = tokio::sync::mpsc::Sender<BusMessage>;
pub type DtuToAlarmRx = tokio::sync::mpsc::Receiver<BusMessage>;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationError {
    pub field: String,
    pub message: String,
    pub value: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationResult {
    pub valid: bool,
    pub errors: Vec<ValidationError>,
    pub corrected_reading: Option<SensorReading>,
}
