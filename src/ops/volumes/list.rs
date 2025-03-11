use crate::fly_rust::volumes::get_volumes;
use crate::ops::Ops;
use crate::state::{RdrResult, ResourceUpdate};
use crate::transformations::ResourceList;

pub async fn list(ops: &Ops, app: &str) -> RdrResult<()> {
    let volumes = get_volumes(&ops.request_builder_machines, app).await?;

    // Sort by id
    let mut sorted_volumes = volumes;
    sorted_volumes.sort_by(|m1, m2| m1.id.cmp(&m2.id));

    let resource_list_tx = {
        let state = ops.shared_state.lock().unwrap();
        state.resource_list_tx.clone()
    };

    // Send the update through channel
    if let Some(resource_list_tx) = resource_list_tx {
        let _ = resource_list_tx
            .send(ResourceUpdate::Volumes(sorted_volumes.transform()))
            .await;
    }

    Ok(())
}
