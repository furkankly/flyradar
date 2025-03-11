use crate::fly_rust::resource_secrets::get_all_app_secrets;
use crate::ops::Ops;
use crate::state::{RdrResult, ResourceUpdate};
use crate::transformations::ResourceList;

pub async fn list(ops: &Ops, app: &str) -> RdrResult<()> {
    let secrets = get_all_app_secrets(&ops.request_builder_graphql, app.to_string()).await?;

    let resource_list_tx = {
        let state = ops.shared_state.lock().unwrap();
        state.resource_list_tx.clone()
    };

    if let Some(resource_list_tx) = resource_list_tx {
        let _ = resource_list_tx
            .send(ResourceUpdate::Secrets(secrets.transform()))
            .await;
    }

    Ok(())
}
