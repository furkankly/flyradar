use crate::fly_rust::resource_apps::list_all;
use crate::ops::{IoRespEvent, Ops};
use crate::state::RdrResult;
use crate::transformations::ResourceList;

pub async fn list(ops: &Ops, seq_id: u64, org_slug: String) -> RdrResult<()> {
    let apps = list_all(&ops.request_builder_graphql).await?;
    let filtered_apps = apps
        .into_iter()
        .filter(|app| app.org == org_slug)
        .collect::<Vec<_>>();

    ops.io_resp_tx
        .send(IoRespEvent::Apps {
            seq_id,
            list: filtered_apps.transform(),
        })
        .await?;

    Ok(())
}
