use std::time::Duration;

use color_eyre::eyre::eyre;

use crate::fly_rust::machine_types::StopMachineInput;
use crate::fly_rust::machines::stop_machine;
use crate::ops::lease::{acquire_leases, ReleaseGuard};
use crate::ops::select_many_machines::select_many_machines;
use crate::ops::Ops;
use crate::state::RdrResult;

//INFO: No --wait-timeout
pub async fn stop(
    ops: &Ops,
    app_name: &str,
    machines: Vec<String>,
    mut params: StopMachineInput,
) -> RdrResult<()> {
    let machines = select_many_machines(ops, app_name, machines).await?;
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

    let _feedback_tx = ops.show_delayed_feedback(
        String::from("Stopping the selected machines..."),
        Duration::from_secs(0),
    );

    for lease in leases {
        let (id, nonce) = {
            let machine = lease.lock().unwrap();
            (machine.id.clone(), machine.lease_nonce.clone().unwrap())
        };
        params.id = id;
        stop_machine(&ops.request_builder_machines, app_name, &params, &nonce).await?;
    }

    Ok(())
}
