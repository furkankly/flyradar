use backon::{ConstantBuilder, Retryable};
use color_eyre::eyre::eyre;
use graphql_client::{GraphQLQuery, Response};
use tracing::{info, instrument};

use super::request_builder::{find_err, RequestBuilderGraphql};
use crate::state::RdrResult;
use crate::transformations::ListSecret;

pub async fn get_all_app_secrets(
    request_builder_graphql: &RequestBuilderGraphql,
    app_name: String,
) -> RdrResult<Vec<ListSecret>> {
    let mut all_secrets = vec![];
    if let Some(response) = get_app_secrets(request_builder_graphql, app_name).await? {
        all_secrets.extend(response.app.secrets.iter().map(|secret| ListSecret {
            name: secret.name.clone(),
            digest: secret.digest.clone(),
            created_at: secret.created_at.clone(),
        }));
    }
    info!("List of secrets: {:#?}", all_secrets);
    Ok(all_secrets)
}

/// Get App Secrets
#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "src/fly_rust/queries/get_app_secrets_schema.graphql",
    query_path = "src/fly_rust/queries/get_app_secrets.graphql",
    response_derives = "Debug"
)]
pub struct GetAppSecrets;
#[instrument(err)]
pub async fn get_app_secrets(
    request_builder_graphql: &RequestBuilderGraphql,
    app_name: String,
) -> RdrResult<Option<get_app_secrets::ResponseData>> {
    let variables = get_app_secrets::Variables { app_name };
    let request_body = GetAppSecrets::build_query(variables);

    let response = (|| async {
        request_builder_graphql
            .query()
            .json(&request_body)
            .send()
            .await
    })
    .retry(ConstantBuilder::default())
    .when(|e| find_err(e, "connection closed before message completed"))
    .await?;

    let bytes = response.bytes().await?;
    let response_body: Response<get_app_secrets::ResponseData> =
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

/// Unset Secrets
#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "src/fly_rust/queries/unset_secrets_schema.graphql",
    query_path = "src/fly_rust/queries/unset_secrets.graphql",
    response_derives = "Debug"
)]
pub struct UnsetSecrets;
#[instrument(err)]
pub async fn unset_secrets(
    request_builder_graphql: &RequestBuilderGraphql,
    app_name: String,
    keys: Vec<String>,
) -> RdrResult<Option<unset_secrets::ResponseData>> {
    let variables = unset_secrets::Variables {
        input: unset_secrets::UnsetSecretsInput {
            app_id: app_name,
            keys,
        },
    };
    let request_body = UnsetSecrets::build_query(variables);
    let response = request_builder_graphql
        .query()
        .json(&request_body)
        .send()
        .await?;
    let bytes = response.bytes().await?;
    let response_body: Response<unset_secrets::ResponseData> =
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
