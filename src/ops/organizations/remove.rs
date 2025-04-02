use crate::fly_rust::resource_organizations::{
    delete_organization_membership, get_detailed_organization_by_slug,
};
use crate::ops::{IoRespEvent, Ops};
use crate::state::{PopupType, RdrResult};

pub async fn remove(ops: &Ops, org_slug: String, email: String) -> RdrResult<()> {
    let resp =
        get_detailed_organization_by_slug(&ops.request_builder_graphql, org_slug.clone()).await?;
    if let Some(resp) = resp {
        let org_id = resp.organizationdetails.id;
        let org_name = resp.organizationdetails.name;
        let user_id = resp
            .organizationdetails
            .members
            .edges
            .iter()
            .find(|edge| edge.node.email == email)
            .map(|edge| edge.node.id.clone());

        match user_id {
            Some(user_id) => {
                delete_organization_membership(&ops.request_builder_graphql, org_id, user_id)
                    .await?;
                ops.io_resp_tx
                    .send(IoRespEvent::SetPopup {
                        popup_type: PopupType::InfoPopup,
                        message: format!(
                            "Successfully removed user {} from {}.

Offboarding Checklist: https://fly.io/dashboard/{}/offboarding",
                            email, org_name, org_slug
                        ),
                    })
                    .await?;
            }
            None => {
                ops.io_resp_tx
                    .send(IoRespEvent::SetPopup {
                        popup_type: PopupType::ErrorPopup,
                        message: format!("User {} not found.", email),
                    })
                    .await?;
            }
        }
    }

    Ok(())
}
