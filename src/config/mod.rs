use serde::Deserialize;

use crate::wireguard::WireGuardStates;

pub mod file;
pub mod helpers;

pub const DEFAULT_API_BASE_URL: &str = "https://api.fly.io";
pub const DEFAULT_FLAPS_BASE_URL: &str = "https://api.machines.dev";
pub const WIREGUARD_STATE_FILE_KEY: &str = "wire_guard_state";

#[derive(Debug, Deserialize)]
pub struct TokenConfig {
    pub access_token: String,
}

#[derive(Debug, Deserialize)]
pub struct FullConfig {
    pub token_config: TokenConfig,
    pub wire_guard_state: Option<WireGuardStates>,
}
