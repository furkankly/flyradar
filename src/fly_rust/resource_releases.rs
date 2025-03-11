use color_eyre::eyre::eyre;
use graphql_client::{GraphQLQuery, Response};
use tracing::instrument;

use super::request_builder::RequestBuilderGraphql;
use crate::state::RdrResult;

/// Get App Releases Machines
#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "src/fly_rust/queries/get_app_releases_machines_schema.graphql",
    query_path = "src/fly_rust/queries/get_app_releases_machines.graphql",
    response_derives = "Debug"
)]
pub struct GetAppReleasesMachines;
#[instrument(err)]
pub async fn get_app_releases_machines(
    request_builder_graphql: &RequestBuilderGraphql,
    app_name: String,
    limit: i64,
) -> RdrResult<Option<get_app_releases_machines::ResponseData>> {
    let variables = get_app_releases_machines::Variables { app_name, limit };
    let request_body = GetAppReleasesMachines::build_query(variables);
    let response = request_builder_graphql
        .query()
        .json(&request_body)
        .send()
        .await?;

    let bytes = response.bytes().await?;
    let response_body: Response<get_app_releases_machines::ResponseData> =
        serde_path_to_error::deserialize(&mut serde_json::Deserializer::from_slice(&bytes))?;
    if let Some(errors) = response_body.errors {
        return Err(eyre!(
            "{}",
            errors
                .iter()
                .map(|e| e.message.clone())
                .collect::<Vec<_>>()
                .join(", "),
        ));
    }
    Ok(response_body.data)
}
