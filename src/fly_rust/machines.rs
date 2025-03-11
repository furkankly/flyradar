use std::fmt::Debug;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use backon::{ConstantBuilder, ExponentialBuilder, Retryable};
use reqwest::StatusCode;
use serde::de::DeserializeOwned;
use serde::Serialize;
use serde_json::json;
use tracing::{error, info, instrument};

use super::machine_types::{
    Machine, MachineLease, RemoveMachineInput, RestartMachineInput, StopMachineInput,
};
use super::request_builder::RequestBuilderMachines;
use crate::fly_rust::request_builder::find_err;
use crate::state::RdrResult;

const NONCE_HEADER: &str = "fly-machine-lease-nonce";

/// Acquire Lease
#[instrument(err)]
pub async fn acquire_lease(
    request_builder_machines: &RequestBuilderMachines,
    app_name: &str,
    machine_id: &str,
    ttl: Option<u64>,
) -> RdrResult<MachineLease> {
    let response = (|| async {
        request_builder_machines
            .post(format!("/v1/apps/{app_name}/machines/{machine_id}/lease"))
            .query(&[("ttl", ttl)])
            .send()
            .await?
            .error_for_status()
    })
    .retry(ExponentialBuilder::default())
    .sleep(tokio::time::sleep)
    .when(|e| {
        if let Some(status) = e.status() {
            return status == StatusCode::CONFLICT;
        }
        false
    })
    .notify(|err, dur| {
        error!("Retrying {:?} after {:?}", err, dur);
    })
    .await?;
    let bytes = response.bytes().await?;
    let lease: MachineLease =
        serde_path_to_error::deserialize(&mut serde_json::Deserializer::from_slice(&bytes))?;
    Ok(lease)
}

/// Release Lease
#[instrument(err)]
pub async fn release_lease(
    request_builder_machines: &RequestBuilderMachines,
    app_name: &str,
    machine_id: &str,
    nonce: &str,
) -> RdrResult<()> {
    request_builder_machines
        .delete(format!("/v1/apps/{app_name}/machines/{machine_id}/lease"))
        .header(NONCE_HEADER, nonce)
        .send()
        .await?
        .error_for_status()?;
    Ok(())
}

/// List Machines
#[instrument(err)]
pub async fn list_machines<T: Debug + DeserializeOwned>(
    request_builder_machines: &RequestBuilderMachines,
    app_name: &str,
    summary: bool,
) -> RdrResult<Vec<T>> {
    let response = (|| async {
        request_builder_machines
            .get(format!("/v1/apps/{app_name}/machines"))
            .query(&[("summary", &summary.to_string())])
            .send()
            .await?
            .error_for_status()
    })
    .retry(ConstantBuilder::default())
    .when(|e| find_err(e, "connection closed before message completed"))
    .await?;

    let bytes = response.bytes().await?;
    let list: Vec<T> =
        serde_path_to_error::deserialize(&mut serde_json::Deserializer::from_slice(&bytes))?;
    info!("List of machines: {:#?}", list);
    Ok(list)
}

pub async fn list_fly_apps_machines(
    request_builder_machines: &RequestBuilderMachines,
    app_name: &str,
) -> RdrResult<(Vec<Machine>, Option<Machine>)> {
    let retry_policy = ExponentialBuilder::default()
        .with_min_delay(Duration::from_millis(500))
        .with_max_times(3);

    let response = (|| async {
        request_builder_machines
            .get(format!("/v1/apps/{app_name}/machines"))
            .send()
            .await?
            .error_for_status()
    })
    .retry(retry_policy)
    // Don't retry any errors except NOT_FOUND
    .when(|e| {
        if let Some(status) = e.status() {
            if status == StatusCode::NOT_FOUND {
                return true; // Continue retrying
            }
        }
        false
    })
    .sleep(tokio::time::sleep)
    .await?;

    let bytes = response.bytes().await?;
    let all_machines: Vec<Machine> =
        serde_path_to_error::deserialize(&mut serde_json::Deserializer::from_slice(&bytes))?;
    let mut machines = Vec::new();
    let mut release_cmd_machine = None;
    for machine in all_machines {
        if machine.is_fly_apps_platform()
            && machine.is_active()
            && !machine.is_fly_apps_release_command()
            && !machine.is_fly_apps_console()
        {
            machines.push(machine);
        } else if machine.is_fly_apps_release_command() {
            release_cmd_machine = Some(machine);
        }
    }
    Ok((machines, release_cmd_machine))
}

/// Get Machine
#[instrument(err)]
pub async fn get_machine(
    request_builder_machines: &RequestBuilderMachines,
    app_name: &str,
    machine_id: &str,
) -> RdrResult<Machine> {
    let response = request_builder_machines
        .get(format!("/v1/apps/{app_name}/machines/{machine_id}"))
        .send()
        .await?
        .error_for_status()?;
    let bytes = response.bytes().await?;
    let machine: Machine =
        serde_path_to_error::deserialize(&mut serde_json::Deserializer::from_slice(&bytes))?;
    Ok(machine)
}

/// Restart Machine
#[derive(Debug, Serialize)]
struct RestartMachineQuery {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signal: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timeout: Option<Duration>,
    pub force_stop: bool,
}
#[instrument(err)]
pub async fn restart_machine(
    request_builder_machines: &RequestBuilderMachines,
    app_name: &str,
    input: &RestartMachineInput,
    nonce: &str,
) -> RdrResult<()> {
    let machine_id = &input.id;
    request_builder_machines
        .post(format!("/v1/apps/{app_name}/machines/{machine_id}/restart"))
        .query(&RestartMachineQuery {
            signal: input.signal.clone(),
            timeout: input.timeout,
            force_stop: input.force_stop,
        })
        .header(NONCE_HEADER, nonce)
        .send()
        .await?
        .error_for_status()?;
    Ok(())
}

#[instrument(err)]
pub async fn start_machine(
    request_builder_machines: &RequestBuilderMachines,
    app_name: &str,
    machine_id: &str,
    nonce: &str,
) -> RdrResult<()> {
    request_builder_machines
        .post(format!("/v1/apps/{app_name}/machines/{machine_id}/start"))
        .header(NONCE_HEADER, nonce)
        .send()
        .await?
        .error_for_status()?;
    Ok(())
}

#[instrument(err)]
pub async fn stop_machine(
    request_builder_machines: &RequestBuilderMachines,
    app_name: &str,
    input: &StopMachineInput,
    nonce: &str,
) -> RdrResult<()> {
    let machine_id = &input.id;
    request_builder_machines
        .post(format!("/v1/apps/{app_name}/machines/{machine_id}/stop"))
        .header(NONCE_HEADER, nonce)
        .send()
        .await?
        .error_for_status()?;
    Ok(())
}

#[instrument(err)]
pub async fn kill_machine(
    request_builder_machines: &RequestBuilderMachines,
    app_name: &str,
    machine_id: &str,
) -> RdrResult<()> {
    let body = json!({
        "signal": 9
    });
    request_builder_machines
        .post(format!("/v1/apps/{app_name}/machines/{machine_id}/signal"))
        .json(&body)
        .send()
        .await?
        .error_for_status()?;
    Ok(())
}

#[instrument(err)]
pub async fn suspend_machine(
    request_builder_machines: &RequestBuilderMachines,
    app_name: &str,
    machine_id: &str,
    nonce: &str,
) -> RdrResult<()> {
    request_builder_machines
        .post(format!("/v1/apps/{app_name}/machines/{machine_id}/suspend"))
        .header(NONCE_HEADER, nonce)
        .send()
        .await?
        .error_for_status()?;
    Ok(())
}

/// Destroy Machine
#[instrument(err)]
pub async fn destroy_machine(
    request_builder_machines: &RequestBuilderMachines,
    app_name: &str,
    machine_id: &str,
    input: RemoveMachineInput,
    nonce: &str,
) -> RdrResult<()> {
    request_builder_machines
        .delete(format!("/v1/apps/{app_name}/machines/{machine_id}"))
        .query(&[("kill", &input.kill.to_string())])
        .header(NONCE_HEADER, nonce)
        .send()
        .await?
        .error_for_status()?;
    Ok(())
}

#[instrument(err)]
pub async fn cordon_machine(
    request_builder_machines: &RequestBuilderMachines,
    app_name: &str,
    machine_id: &str,
    nonce: &str,
) -> RdrResult<()> {
    request_builder_machines
        .post(format!("/v1/apps/{app_name}/machines/{machine_id}/cordon"))
        .header(NONCE_HEADER, nonce)
        .send()
        .await?
        .error_for_status()?;
    Ok(())
}

#[instrument(err)]
pub async fn uncordon_machine(
    request_builder_machines: &RequestBuilderMachines,
    app_name: &str,
    machine_id: &str,
    nonce: &str,
) -> RdrResult<()> {
    request_builder_machines
        .post(format!(
            "/v1/apps/{app_name}/machines/{machine_id}/uncordon"
        ))
        .header(NONCE_HEADER, nonce)
        .send()
        .await?
        .error_for_status()?;
    Ok(())
}

const PROXY_TIMEOUT_THRESHOLD: Duration = Duration::from_secs(60);
pub async fn wait(
    request_builder_machines: &RequestBuilderMachines,
    app_name: &str,
    machine: &Arc<Mutex<Machine>>,
    state: &str,
    timeout: Duration,
) -> Result<(), reqwest::Error> {
    let mut state = String::from(state);
    info!("STATE: {}", state);
    if state.is_empty() {
        state = String::from("started");
    }
    let (machine_id, instance_id) = {
        let machine_guard = machine.lock().unwrap();
        let instance_id = if let Some(version) = machine_guard.version.clone() {
            version
        } else {
            machine_guard.instance_id.clone()
        };
        (machine_guard.id.clone(), instance_id)
    };

    let timeout_seconds = if timeout < Duration::from_secs(1) {
        Duration::from_secs(1)
    } else if timeout > PROXY_TIMEOUT_THRESHOLD {
        PROXY_TIMEOUT_THRESHOLD
    } else {
        timeout
    };

    request_builder_machines
        .get(format!("/v1/apps/{app_name}/machines/{machine_id}"))
        .query(&[
            ("instance_id", &instance_id),
            ("timeout_seconds", &timeout_seconds.as_secs().to_string()),
            ("state", &state),
        ])
        .send()
        .await?
        .error_for_status()?;

    Ok(())
}
