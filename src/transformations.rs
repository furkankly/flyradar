use std::fmt;

use chrono::{DateTime, Utc};
use chrono_humanize::HumanTime;
use serde::Deserialize;
use timeago::{Formatter, TimeUnit};

// INFO: Intermediary types to select fields to show in the table.
// id is needed to be able to render the selected state optimistically in case of deletions happen in
// between fetches
#[derive(Debug)]
pub struct ListOrganization {
    pub id: String,
    pub slug: String,
    pub name: String,
    pub viewer_role: String,
    pub type_: String,
}
#[derive(Debug)]
pub struct ListApp {
    pub id: String,
    pub name: String,
    pub org: String,
    pub status: String,
    pub latest_deploy: String,
}
#[derive(Debug, Deserialize)]
pub struct ListMachine {
    pub id: String,
    pub name: String,
    pub state: String,
    pub region: String,
    pub updated_at: String,
}
#[derive(Debug, Deserialize)]
pub struct ListVolume {
    pub id: String,
    pub state: String,
    pub name: String,
    pub size_gb: i32,
    pub region: String,
    pub zone: String,
    pub encrypted: bool,
    pub attached_machine_id: Option<String>,
    pub created_at: String,
}
#[derive(Debug, Deserialize)]
pub struct ListSecret {
    pub name: String,
    pub digest: String,
    pub created_at: String,
}

pub fn format_time(time: &str) -> String {
    let time = DateTime::parse_from_rfc3339(time)
        .unwrap()
        .with_timezone(&Utc);
    let now = Utc::now();
    let duration = now.signed_duration_since(time);

    if duration.num_days() > 7 {
        return time.format("%b %d %Y %H:%M").to_string();
    }

    Formatter::new()
        .min_unit(TimeUnit::Seconds)
        .convert_chrono(time, now)
}

impl From<&ListOrganization> for Vec<String> {
    fn from(org: &ListOrganization) -> Self {
        vec![
            org.id.clone(),
            org.name.clone(),
            org.viewer_role.clone(),
            org.slug.clone(),
            org.type_.clone(),
        ]
    }
}

impl From<Vec<String>> for ListOrganization {
    fn from(vec: Vec<String>) -> Self {
        ListOrganization {
            id: vec[0].clone(),
            name: vec[1].clone(),
            viewer_role: vec[2].clone(),
            slug: vec[3].clone(),
            type_: vec[4].clone(),
        }
    }
}

impl From<&ListApp> for Vec<String> {
    fn from(app: &ListApp) -> Self {
        vec![
            app.id.clone(),
            app.name.clone(),
            app.org.to_string(),
            app.status.clone(),
            if app.latest_deploy.is_empty() {
                app.latest_deploy.clone()
            } else {
                format_time(&app.latest_deploy)
            },
        ]
    }
}

impl From<Vec<String>> for ListApp {
    fn from(vec: Vec<String>) -> Self {
        ListApp {
            id: vec[0].clone(),
            name: vec[1].clone(),
            org: vec[2].clone(),
            status: vec[3].clone(),
            latest_deploy: vec[4].clone(),
        }
    }
}

impl From<&ListMachine> for Vec<String> {
    fn from(machine: &ListMachine) -> Self {
        vec![
            machine.id.clone(),
            machine.name.clone(),
            machine.state.clone(),
            machine.region.clone(),
            if machine.updated_at.is_empty() {
                machine.updated_at.clone()
            } else {
                format_time(&machine.updated_at)
            },
        ]
    }
}

impl From<Vec<String>> for ListMachine {
    fn from(vec: Vec<String>) -> Self {
        ListMachine {
            id: vec[0].clone(),
            name: vec[1].clone(),
            state: vec[2].clone(),
            region: vec[3].clone(),
            updated_at: vec[4].clone(),
        }
    }
}

impl From<&ListVolume> for Vec<String> {
    fn from(volume: &ListVolume) -> Self {
        let mut created_at = String::new();
        if !&volume.created_at.is_empty() {
            let time = DateTime::parse_from_rfc3339(&volume.created_at)
                .unwrap()
                .with_timezone(&Utc);
            created_at = HumanTime::from(time).to_string();
        };

        vec![
            volume.id.clone(),
            volume.state.clone(),
            volume.name.clone(),
            volume.size_gb.to_string() + "GB",
            volume.region.clone(),
            volume.zone.clone(),
            volume.encrypted.to_string(),
            volume.attached_machine_id.clone().unwrap_or_default(),
            created_at,
        ]
    }
}

impl From<Vec<String>> for ListVolume {
    fn from(vec: Vec<String>) -> Self {
        ListVolume {
            id: vec[0].clone(),
            state: vec[1].clone(),
            name: vec[2].clone(),
            size_gb: vec[3].trim_end_matches("GB").parse::<i32>().unwrap(),
            region: vec[4].clone(),
            zone: vec[5].clone(),
            encrypted: vec[6].parse::<bool>().unwrap(),
            attached_machine_id: Some(vec[7].clone()),
            created_at: vec[8].clone(),
        }
    }
}

impl From<&ListSecret> for Vec<String> {
    fn from(secret: &ListSecret) -> Self {
        vec![
            secret.name.clone(),
            secret.digest.clone(),
            if secret.created_at.is_empty() {
                secret.created_at.clone()
            } else {
                format_time(&secret.created_at)
            },
        ]
    }
}

impl From<Vec<String>> for ListSecret {
    fn from(vec: Vec<String>) -> Self {
        ListSecret {
            name: vec[0].clone(),
            digest: vec[1].clone(),
            created_at: vec[2].clone(),
        }
    }
}

/// items of SelectableList
pub trait ResourceList: fmt::Debug + Send + Sync {
    fn transform(&self) -> Vec<Vec<String>>;
}

impl ResourceList for Vec<ListOrganization> {
    fn transform(&self) -> Vec<Vec<String>> {
        self.iter().map(Vec::<String>::from).collect()
    }
}

impl ResourceList for Vec<ListApp> {
    fn transform(&self) -> Vec<Vec<String>> {
        self.iter().map(Vec::<String>::from).collect()
    }
}

impl ResourceList for Vec<ListMachine> {
    fn transform(&self) -> Vec<Vec<String>> {
        self.iter().map(Vec::<String>::from).collect()
    }
}

impl ResourceList for Vec<ListVolume> {
    fn transform(&self) -> Vec<Vec<String>> {
        self.iter().map(Vec::<String>::from).collect()
    }
}

impl ResourceList for Vec<ListSecret> {
    fn transform(&self) -> Vec<Vec<String>> {
        self.iter().map(Vec::<String>::from).collect()
    }
}
