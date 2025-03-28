use crate::fly_rust::resource_organizations::{get_all_organizations, OrganizationFilter};
use crate::ops::{IoRespEvent, Ops};
use crate::state::RdrResult;
use crate::transformations::ResourceList;

pub async fn list(ops: &Ops, filter: OrganizationFilter) -> RdrResult<()> {
    let organizations = get_all_organizations(&ops.request_builder_graphql, filter).await?;

    ops.io_resp_tx
        .send(IoRespEvent::Organizations {
            list: organizations.transform(),
        })
        .await?;

    Ok(())
}
