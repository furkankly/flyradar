use color_eyre::eyre::eyre;
use graphql_client::{GraphQLQuery, Response};
use tracing::instrument;

use super::request_builder::RequestBuilderGraphql;
use crate::state::RdrResult;

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "src/fly_rust/queries/validate_wire_guard_peers_schema.graphql",
    query_path = "src/fly_rust/queries/validate_wire_guard_peers.graphql",
    response_derives = "Debug"
)]
struct ValidateWireGuardPeers;
#[instrument(err)]
pub async fn validate_wire_guard_peers(
    request_builder_graphql: &RequestBuilderGraphql,
    app_name: String,
    peer_ips: Vec<String>,
) -> RdrResult<Option<validate_wire_guard_peers::ResponseData>> {
    let variables = validate_wire_guard_peers::Variables {
        input: validate_wire_guard_peers::ValidateWireGuardPeersInput { peer_ips },
    };
    let request_body = ValidateWireGuardPeers::build_query(variables);
    let response = request_builder_graphql
        .query()
        .json(&request_body)
        .send()
        .await?;
    let response_body: Response<validate_wire_guard_peers::ResponseData> = response.json().await?;
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
