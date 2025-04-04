use crate::fly_rust::resource_organizations::delete_organization;
use crate::ops::Ops;
use crate::state::RdrResult;

pub async fn delete(ops: &Ops, org_id: String) -> RdrResult<()> {
    delete_organization(&ops.request_builder_graphql, org_id).await?;
    Ok(())
}
