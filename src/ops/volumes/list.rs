use crate::fly_rust::volumes::get_volumes;
use crate::ops::{IoRespEvent, Ops};
use crate::state::RdrResult;
use crate::transformations::ResourceList;

pub async fn list(ops: &Ops, seq_id: u64, app: &str) -> RdrResult<()> {
    let mut volumes = get_volumes(&ops.request_builder_machines, app).await?;
    // Sort by id
    volumes.sort_by(|m1, m2| m1.id.cmp(&m2.id));

    ops.io_resp_tx
        .send(IoRespEvent::Volumes {
            seq_id,
            list: volumes.transform(),
        })
        .await?;

    Ok(())
}
