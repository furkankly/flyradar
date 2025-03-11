use chrono::{DateTime, Utc};
use serde::Deserialize;

#[derive(Deserialize, Debug)]
pub struct Volume {
    pub id: String,
    pub name: String,
    pub state: String,
    pub size_gb: i32,
    pub region: String,
    pub zone: String,
    pub encrypted: bool,
    #[serde(rename = "attached_machine_id")]
    pub attached_machine: Option<String>,
    #[serde(rename = "attached_alloc_id")]
    pub attached_allocation: Option<String>,
    pub created_at: DateTime<Utc>,
    #[serde(default)]
    pub host_dedication_id: Option<String>,
    pub snapshot_retention: i32,
    pub auto_backup_enabled: bool,
    #[serde(default)]
    pub host_status: String,
}

impl Volume {
    pub fn is_attached(&self) -> bool {
        self.attached_machine.is_some() || self.attached_allocation.is_some()
    }
}

#[derive(Debug)]
pub struct RemoveVolumeInput {
    pub id: String,
}
