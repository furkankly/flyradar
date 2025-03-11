use serde::Deserialize;
use tracing::instrument;

use super::request_builder::RequestBuilderFly;
use crate::logs::entry::LogEntry;
use crate::state::RdrResult;

#[derive(Deserialize)]
#[allow(dead_code)]
struct Data {
    id: String,
    attributes: LogEntry,
}

#[derive(Deserialize)]
struct Meta {
    next_token: String,
}

#[derive(Deserialize)]
struct GetLogsResponse {
    data: Vec<Data>,
    meta: Meta,
}

#[instrument(err)]
pub async fn get_app_logs(
    request_builder_fly: &RequestBuilderFly,
    app_name: &str,
    next_token: Option<String>,
    instance_id: Option<String>,
    region: Option<String>,
) -> RdrResult<(Vec<LogEntry>, String)> {
    let response = request_builder_fly
        .get(format!("/v1/apps/{app_name}/logs"))
        .query(&[
            ("next_token", next_token),
            ("instance", instance_id),
            ("region", region),
        ])
        .send()
        .await?
        .error_for_status()?;

    let bytes = response.bytes().await?;
    let response: GetLogsResponse =
        serde_path_to_error::deserialize(&mut serde_json::Deserializer::from_slice(&bytes))?;

    let entries: Vec<LogEntry> = response.data.iter().map(|d| d.attributes.clone()).collect();
    let next_token = response.meta.next_token;

    Ok((entries, next_token))
}
