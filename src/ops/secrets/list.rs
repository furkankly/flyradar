use crate::fly_rust::resource_secrets::get_all_app_secrets;
use crate::ops::{IoRespEvent, Ops};
use crate::state::RdrResult;
use crate::transformations::ResourceList;

pub async fn list(ops: &Ops, seq_id: u64, app: &str) -> RdrResult<()> {
    let secrets = get_all_app_secrets(&ops.request_builder_graphql, app.to_string()).await?;

    ops.io_resp_tx
        .send(IoRespEvent::Secrets {
            seq_id,
            list: secrets.transform(),
        })
        .await?;

    Ok(())
}
