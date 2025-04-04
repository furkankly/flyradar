use crate::fly_rust::resource_organizations::create_organization_invite;
use crate::ops::{IoRespEvent, Ops};
use crate::state::{PopupType, RdrResult};

pub async fn invite(ops: &Ops, org_id: String, email: String) -> RdrResult<()> {
    let resp = create_organization_invite(&ops.request_builder_graphql, org_id, email).await?;
    if let Some(resp) = resp {
        if let Some(invitation) = resp.create_organization_invitation {
            let slug = invitation.invitation.organization.slug;
            let email = invitation.invitation.email;
            // let redeemed = invitation.invitation.redeemed;
            ops.io_resp_tx
                .send(IoRespEvent::SetPopup {
                    popup_type: PopupType::InfoPopup,
                    message: format!(
                        "Invitation sent. 
Organization: {}
Email: {}",
                        slug, email,
                    ),
                })
                .await?;
        }
    }
    Ok(())
}
