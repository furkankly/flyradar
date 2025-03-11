use std::fmt::Display;

use serde::{Deserialize, Serialize};

use crate::widgets::log_viewer::Level;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LogEntry {
    pub level: String,
    pub instance: String,
    pub message: String,
    pub region: String,
    pub timestamp: String,
    pub meta: Meta,
}
impl LogEntry {
    pub fn map_level(&self) -> Level {
        match self.level.as_str() {
            "error" => Level::Error,
            "warn" => Level::Warn,
            "info" => Level::Info,
            "debug" => Level::Debug,
            "trace" => Level::Trace,
            _ => Level::Trace,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Meta {
    pub instance: String,
    pub region: String,
    pub event: Event,
    pub http: Option<Http>,
    pub error: Option<Error>,
    pub url: Option<Url>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Event {
    pub provider: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Http {
    pub request: Request,
    pub response: Response,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Request {
    pub id: String,
    pub method: String,
    pub version: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Response {
    pub status_code: i32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Error {
    pub code: i32,
    pub message: String,
}

impl Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Url {
    pub full: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NatsLog {
    pub event: NatsEvent,
    pub fly: NatsFly,
    pub host: String,
    pub log: NatsLogLevel,
    pub message: String,
    pub timestamp: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NatsEvent {
    pub provider: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NatsFly {
    pub app: NatsApp,
    pub region: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NatsApp {
    pub instance: String,
    pub name: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NatsLogLevel {
    pub level: String,
}
