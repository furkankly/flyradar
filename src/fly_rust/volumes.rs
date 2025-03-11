use std::fmt::Debug;

use backon::{ConstantBuilder, Retryable};
use serde::de::DeserializeOwned;
use tracing::{info, instrument};

use super::request_builder::{find_err, RequestBuilderMachines};
use super::volume_types::Volume;
use crate::state::RdrResult;
use crate::transformations::ListVolume;

pub const DESTROYED_VOLUME_STATES: [&str; 5] = [
    "scheduling_destroy",
    "fork_cleanup",
    "waiting_for_detach",
    "pending_destroy",
    "destroying",
];

/// List Volumes
#[instrument(err)]
pub async fn get_all_volumes<T: Debug + DeserializeOwned>(
    request_builder_machines: &RequestBuilderMachines,
    app_name: &str,
) -> RdrResult<Vec<T>> {
    let response = (|| async {
        request_builder_machines
            .get(format!("/v1/apps/{app_name}/volumes"))
            .send()
            .await?
            .error_for_status()
    })
    .retry(ConstantBuilder::default())
    .when(|e| find_err(e, "connection closed before message completed"))
    .await?;

    let bytes = response.bytes().await?;
    let list: Vec<T> =
        serde_path_to_error::deserialize(&mut serde_json::Deserializer::from_slice(&bytes))?;
    Ok(list)
}

pub async fn get_volumes(
    request_builder_machines: &RequestBuilderMachines,
    app_name: &str,
) -> RdrResult<Vec<ListVolume>> {
    let mut volumes = get_all_volumes::<ListVolume>(request_builder_machines, app_name).await?;
    volumes.retain(|volume| !DESTROYED_VOLUME_STATES.contains(&volume.state.as_str()));
    info!("List of volumes: {:#?}", volumes);
    Ok(volumes)
}

pub async fn delete_volume(
    request_builder_machines: &RequestBuilderMachines,
    app_name: &str,
    volume_id: String,
) -> RdrResult<Option<Volume>> {
    let response = request_builder_machines
        .delete(format!("/v1/apps/{app_name}/volumes/{volume_id}"))
        .send()
        .await?
        .error_for_status()?;
    let bytes = response.bytes().await?;
    let volume: Option<Volume> =
        serde_path_to_error::deserialize(&mut serde_json::Deserializer::from_slice(&bytes))?;
    Ok(volume)
}
