use crate::fly_rust::resource_organizations::get_detailed_organization_by_slug;
use crate::ops::{IoRespEvent, Ops};
use crate::state::RdrResult;

pub async fn members(ops: &Ops, org_slug: String) -> RdrResult<()> {
    let org = get_detailed_organization_by_slug(&ops.request_builder_graphql, org_slug).await?;

    if let Some(org) = org {
        let members = org
            .organizationdetails
            .members
            .edges
            .into_iter()
            .map(|edge| vec![edge.node.name, edge.node.email, edge.role])
            .collect::<Vec<Vec<String>>>();
        ops.io_resp_tx
            .send(IoRespEvent::OrganizationMembers { list: members })
            .await?;
    }

    Ok(())
}
