use crate::fly_rust::resource_releases::get_app_releases_machines;
use crate::ops::{IoRespEvent, Ops};
use crate::state::RdrResult;
use crate::transformations::format_time;

pub async fn releases(ops: &Ops, app_name: String, limit: i64) -> RdrResult<()> {
    let response = get_app_releases_machines(&ops.request_builder_graphql, app_name, limit).await?;
    if let Some(response) = response {
        let app_releases_list = response
            .app
            .releases
            .nodes
            .iter()
            .map(|release| {
                vec![
                    release.version.to_string(),
                    release.status.clone(),
                    release.description.clone(),
                    release.user.email.clone(),
                    format_time(&release.created_at),
                    release.image_ref.clone(),
                ]
            })
            .collect();
        ops.io_resp_tx
            .send(IoRespEvent::AppReleases {
                list: app_releases_list,
            })
            .await?;
    }

    Ok(())
}
