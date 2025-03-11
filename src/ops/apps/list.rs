use crate::fly_rust::resource_apps::list_all;
use crate::ops::Ops;
use crate::state::{RdrResult, ResourceUpdate};
use crate::transformations::ResourceList;

pub async fn list(ops: &Ops) -> RdrResult<()> {
    let apps = list_all(&ops.request_builder_graphql).await?;

    let resource_list_tx = {
        let state = ops.shared_state.lock().unwrap();
        state.resource_list_tx.clone()
    };

    if let Some(resource_list_tx) = resource_list_tx {
        let _ = resource_list_tx
            .send(ResourceUpdate::Apps(apps.transform()))
            .await;
    }

    Ok(())
}
