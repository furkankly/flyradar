use std::sync::{Arc, Mutex};

use color_eyre::eyre::{eyre, OptionExt};

use crate::fly_rust::machine_types::RemoveMachineInput;
use crate::fly_rust::machines::{destroy_machine, get_machine};
use crate::ops::lease::{acquire, ReleaseGuard};
use crate::ops::Ops;
use crate::state::RdrResult;

//TODO: Best effort post deletion hook that unregisters pg.(?)
pub async fn destroy(ops: &Ops, app_name: &str, params: RemoveMachineInput) -> RdrResult<()> {
    let machine = get_machine(&ops.request_builder_machines, app_name, &params.id).await?;
    match machine.state.as_str() {
        "stopped" | "suspended" => {
            //destroy
        }
        "destroyed" => {
            if !params.kill {
                return Err(eyre!("Machine {} has already been destroyed.", machine.id));
            }
        }
        "started" => {
            if !params.kill {
                return Err(eyre!(
                    "Machine {} currently started, either stop first or force destroy it.",
                    machine.id
                ));
            }
        }
        _ => {
            if !params.kill {
                return Err(eyre!(
"Machine {} is in a {} state and cannot be destroyed since it is not stopped or suspended, either stop first or force destroy it.", machine.id, machine.state)
                );
            }
        }
    }
    let (machine, release) = acquire(ops, app_name, Arc::new(Mutex::new(machine))).await?;
    let _release_guard = ReleaseGuard {
        release: Some(release),
    };
    let (machine_id, lease_nonce) = {
        let machine_guard = machine.lock().unwrap();
        (machine_guard.id.clone(), machine_guard.lease_nonce.clone())
    };
    let lease_nonce = lease_nonce.ok_or_eyre("Nonce not found trying to destroy the machine.")?;
    destroy_machine(
        &ops.request_builder_machines,
        app_name,
        &machine_id,
        params,
        &lease_nonce,
    )
    .await?;
    Ok(())
}
