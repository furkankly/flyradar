use std::time::Duration;

use color_eyre::eyre::eyre;

use crate::fly_rust::machine_types::{RestartMachineInput, MACHINE_STATE_STARTED};
use crate::fly_rust::machines::list_fly_apps_machines;
use crate::ops::lease::{acquire_leases, ReleaseGuard};
use crate::ops::machines::restart::machine_restart;
use crate::ops::Ops;
use crate::state::RdrResult;

#[derive(Debug, Default)]
pub struct AppRestartParams {
    pub force_stop: bool,
}

pub async fn restart(ops: &Ops, app_name: &str, params: AppRestartParams) -> RdrResult<()> {
    let message = format!("Restarting the machines for {}.", app_name);
    let _feedback_tx = ops.show_delayed_feedback(message, Duration::from_secs(3));

    let (machines, _) = list_fly_apps_machines(&ops.request_builder_machines, app_name).await?;

    let (leases, errors, release) = acquire_leases(ops, app_name, machines).await;
    let _release_guard = ReleaseGuard {
        release: Some(release),
    };

    if !errors.is_empty() {
        return Err(eyre!(
            "{} errors occurred:\n{}",
            errors.len(),
            errors
                .iter()
                .enumerate()
                .map(|(i, e)| format!("{}. {}", i + 1, e))
                .collect::<Vec<_>>()
                .join("\n")
        ));
    }

    let mut restart_params = RestartMachineInput {
        force_stop: params.force_stop,
        ..Default::default()
    };

    for lease in leases {
        let (nonce, state) = {
            let machine = lease.lock().unwrap();
            (machine.lease_nonce.clone().unwrap(), machine.state.clone())
        };
        if state != MACHINE_STATE_STARTED {
            continue;
        }
        machine_restart(
            &ops.request_builder_machines,
            app_name,
            lease,
            &mut restart_params,
            &nonce,
        )
        .await?;
    }
    Ok(())
}
