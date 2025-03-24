use crate::fly_rust::resource_organizations::{get_all_organizations, OrganizationFilter};
use crate::ops::Ops;
use crate::state::{RdrResult, ResourceUpdate};
use crate::transformations::ResourceList;

pub async fn list(ops: &Ops, filter: OrganizationFilter) -> RdrResult<()> {
    let organizations = get_all_organizations(&ops.request_builder_graphql, filter).await?;

    let resource_list_tx = {
        let state = ops.shared_state.lock().unwrap();
        state.resource_list_tx.clone()
    };

    if let Some(resource_list_tx) = resource_list_tx {
        let _ = resource_list_tx
            .send(ResourceUpdate::Organizations(organizations.transform()))
            .await;
    }

    Ok(())
}
