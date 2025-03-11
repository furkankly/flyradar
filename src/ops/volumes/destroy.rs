use crate::fly_rust::volume_types::RemoveVolumeInput;
use crate::fly_rust::volumes::delete_volume;
use crate::ops::Ops;
use crate::state::RdrResult;

pub async fn destroy(ops: &Ops, app_name: &str, params: RemoveVolumeInput) -> RdrResult<()> {
    delete_volume(&ops.request_builder_machines, app_name, params.id).await?;
    Ok(())
}
