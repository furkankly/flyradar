use std::fmt;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use backon::{ExponentialBuilder, Retryable};
use color_eyre::eyre::{bail, eyre};
use tokio::time::timeout;
use tracing::info;

use crate::fly_rust::machine_types::{Machine, MachineRestartPolicy};
use crate::fly_rust::machines::wait;
use crate::fly_rust::request_builder::RequestBuilderMachines;
use crate::state::RdrResult;

pub async fn wait_for_start_or_stop(
    request_builder: &RequestBuilderMachines,
    app_name: &str,
    machine: Arc<Mutex<Machine>>,
    action: &str,
    timeout_duration: Duration,
) -> RdrResult<()> {
    let wait_on_action = match action {
        "start" => "started",
        "stop" => "stopped",
        _ => bail!("invalid action"),
    };

    let retry_policy = ExponentialBuilder::default()
        .with_min_delay(Duration::from_millis(500))
        .with_max_delay(Duration::from_secs(2))
        .with_factor(2.0);

    match timeout(
        timeout_duration,
        (|| async {
            wait(
                request_builder,
                app_name,
                &machine,
                wait_on_action,
                Duration::from_secs(60),
            )
            .await
        })
        .retry(retry_policy)
        .sleep(tokio::time::sleep)
        .notify(|err, _| tracing::error!("Retry attempt failed: {}", err)),
    )
    .await
    {
        Ok(Ok(_)) => {
            info!("Succeded wait for start or stop.");
            Ok(())
        }
        Ok(Err(err)) => {
            let err_str = err.to_string();

            let restart_policy = {
                let machine_guard = machine.lock().unwrap();
                (|| {
                    machine_guard
                        .config
                        .as_ref()?
                        .restart
                        .as_ref()?
                        .policy
                        .clone()
                })()
            };

            if err_str.contains("machine failed to reach desired state")
                && matches!(restart_policy, Some(MachineRestartPolicy::No))
            {
                Err(eyre!("machine failed to reach desired start state, and restart policy was set to {:?} restart",
               restart_policy))
            } else if err_str.contains("status: 400") {
                Err(eyre!("failed waiting for machine: {}", err))
            } else {
                Err(eyre!(err_str))
            }
        }
        Err(_elapsed) => {
            let machine_id = {
                let machine_guard = machine.lock().unwrap();
                machine_guard.id.clone()
            };
            Err(WaitTimeoutError {
                machine_id,
                timeout: timeout_duration,
                desired_state: wait_on_action.to_string(),
            })?
        }
    }
}

#[derive(Debug)]
#[allow(dead_code)]
pub struct WaitTimeoutError {
    pub machine_id: String,
    pub timeout: Duration,
    pub desired_state: String,
}

impl std::error::Error for WaitTimeoutError {}

impl fmt::Display for WaitTimeoutError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "timeout reached waiting for machine's state to change")
    }
}

#[allow(dead_code)]
impl WaitTimeoutError {
    pub fn description(&self) -> String {
        format!(
            "The machine {} took more than {:?} to reach \"{}\"",
            self.machine_id, self.timeout, self.desired_state
        )
    }

    pub fn desired_state(&self) -> &str {
        &self.desired_state
    }
}
