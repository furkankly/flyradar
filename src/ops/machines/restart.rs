use std::sync::{Arc, Mutex};
use std::time::Duration;

use color_eyre::eyre::eyre;

use crate::fly_rust::machine_types::{Machine, RestartMachineInput};
use crate::fly_rust::machines::restart_machine;
use crate::fly_rust::request_builder::RequestBuilderMachines;
use crate::ops::lease::{acquire_leases, ReleaseGuard};
use crate::ops::select_many_machines::select_many_machines;
use crate::ops::wait::wait_for_start_or_stop;
use crate::ops::Ops;
use crate::state::RdrResult;

//TODO: Integrate skip_health_checks
// 	if !input.SkipHealthChecks {
// 		if err := watch.MachinesChecks(ctx, []*fly.Machine{m}); err != nil {
// 			return fmt.Errorf("failed to wait for health checks to pass: %w", err)
// 		}
// 	}
pub async fn machine_restart(
    request_builder_machines: &RequestBuilderMachines,
    app_name: &str,
    machine: Arc<Mutex<Machine>>,
    params: &mut RestartMachineInput,
    nonce: &str,
) -> RdrResult<()> {
    let id = {
        let machine = machine.lock().unwrap();
        machine.id.clone()
    };
    params.id = id;
    restart_machine(request_builder_machines, app_name, params, nonce).await?;
    wait_for_start_or_stop(
        request_builder_machines,
        app_name,
        machine,
        "start",
        Duration::from_secs(300),
    )
    .await?;
    Ok(())
}

pub async fn restart(
    ops: &Ops,
    app_name: &str,
    machines: Vec<String>,
    mut params: RestartMachineInput,
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
        String::from("Restarting the selected machines..."),
        Duration::from_secs(0),
    );

    for lease in leases {
        let nonce = {
            let machine = lease.lock().unwrap();
            machine.lease_nonce.clone().unwrap()
        };
        machine_restart(
            &ops.request_builder_machines,
            app_name,
            lease,
            &mut params,
            &nonce,
        )
        .await?;
    }
    Ok(())
}
