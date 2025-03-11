use crate::fly_rust::machines::list_machines;
use crate::ops::Ops;
use crate::state::{RdrResult, ResourceUpdate};
use crate::transformations::{ListMachine, ResourceList};

pub async fn list(ops: &Ops, app: &str) -> RdrResult<()> {
    let machines = list_machines::<ListMachine>(
        &ops.request_builder_machines,
        app,
        //INFO: When summary is set to true, server doesn't send states like "stopping"
        false,
    )
    .await?;

    // Sort by id
    let mut sorted_machines = machines;
    sorted_machines.sort_by(|m1, m2| m1.id.cmp(&m2.id));

    let resource_list_tx = {
        let state = ops.shared_state.lock().unwrap();
        state.resource_list_tx.clone()
    };

    if let Some(resource_list_tx) = resource_list_tx {
        let _ = resource_list_tx
            .send(ResourceUpdate::Machines(sorted_machines.transform()))
            .await;
    }

    Ok(())
}
