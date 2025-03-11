use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use tracing::debug;

use crate::config::helpers::get_config_file_path;
use crate::config::{self};
use crate::fly_rust::request_builder::RequestBuilderGraphql;
use crate::fly_rust::resource_wireguard::validate_wire_guard_peers;
use crate::state::RdrResult;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Peer {
    #[serde(rename = "peerip")]
    pub peer_ip: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WireGuardState {
    pub peer: Peer,
}

pub type WireGuardStates = HashMap<String, WireGuardState>;

//INFO: Unlike the Go implementation which uses
// Viper, this reads directly from the config file with proper locking.
async fn get_wire_guard_state() -> RdrResult<Option<WireGuardStates>> {
    let config_file_path = get_config_file_path()?;
    config::file::read_wg_state(config_file_path).await
}

async fn set_wire_guard_state(states: WireGuardStates) -> RdrResult<()> {
    let config_file_path = get_config_file_path()?;
    config::file::set_wg_state(config_file_path, states).await
}

pub async fn prune_invalid_peers(
    request_builder_graphql: &RequestBuilderGraphql,
    app_name: String,
) -> RdrResult<()> {
    let states = match get_wire_guard_state().await {
        Ok(states) => states,
        Err(_) => return Ok(()),
    };

    let mut states = match states {
        Some(states) => states,
        None => return Ok(()),
    };

    let peer_ips: Vec<String> = states
        .values()
        .map(|state| state.peer.peer_ip.clone())
        .collect();

    let res = validate_wire_guard_peers(request_builder_graphql, app_name, peer_ips).await?;
    if let Some(res) = res {
        for invalid_ip in res.validate_wire_guard_peers.invalid_peer_ips {
            states.retain(|org_slug, state| {
                if state.peer.peer_ip == invalid_ip {
                    debug!(
                        "removing invalid peer {} for organization {}",
                        invalid_ip, org_slug
                    );
                    false
                } else {
                    true
                }
            });
        }
    }

    set_wire_guard_state(states).await?;

    Ok(())
}
