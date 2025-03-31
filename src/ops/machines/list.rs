use crate::fly_rust::machines::list_machines;
use crate::ops::{IoRespEvent, Ops};
use crate::state::RdrResult;
use crate::transformations::{ListMachine, ResourceList};

pub async fn list(ops: &Ops, seq_id: u64, app: &str) -> RdrResult<()> {
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

    ops.io_resp_tx
        .send(IoRespEvent::Machines {
            seq_id,
            list: sorted_machines.transform(),
        })
        .await?;

    Ok(())
}
