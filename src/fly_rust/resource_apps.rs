use backon::{ConstantBuilder, Retryable};
use color_eyre::eyre::eyre;
use graphql_client::{GraphQLQuery, Response};
use tracing::{info, instrument};

use super::request_builder::{find_err, RequestBuilderGraphql};
use crate::state::RdrResult;
use crate::transformations::ListApp;

/// List All Apps
#[instrument(err)]
pub async fn list_all(request_builder_grapqhl: &RequestBuilderGraphql) -> RdrResult<Vec<ListApp>> {
    let mut all_apps = vec![];
    let mut current_cursor = None;

    loop {
        let page =
            get_apps_page(request_builder_grapqhl, None, None, current_cursor.clone()).await?;
        if let Some(page) = page {
            all_apps.extend(page.apps.nodes.iter().map(|node| {
                let mut latest_deploy = String::from("");
                if node.deployed {
                    if let Some(current_release) = &node.current_release {
                        latest_deploy = current_release.created_at.clone();
                    }
                }
                ListApp {
                    id: node.id.clone(),
                    name: node.name.clone(),
                    org: node.organization.slug.clone(),
                    status: node.status.clone(),
                    latest_deploy,
                }
            }));

            match (
                page.apps.page_info.has_next_page,
                page.apps.page_info.end_cursor,
            ) {
                (true, cursor) => current_cursor = Some(cursor),
                _ => break,
            }
        }
    }
    info!("List of apps: {:#?}", all_apps);
    Ok(all_apps)
}

/// Get Apps Page
#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "src/fly_rust/queries/get_apps_page_schema.graphql",
    query_path = "src/fly_rust/queries/get_apps_page.graphql",
    response_derives = "Debug"
)]
pub struct GetAppsPage;
#[instrument(err)]
pub async fn get_apps_page(
    request_builder_graphql: &RequestBuilderGraphql,
    org: Option<String>,
    role: Option<String>,
    after: Option<String>,
) -> RdrResult<Option<get_apps_page::ResponseData>> {
    let variables = get_apps_page::Variables { org, role, after };
    let request_body = GetAppsPage::build_query(variables);

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
    let response_body: Response<get_apps_page::ResponseData> =
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

/// Get App Compact
#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "src/fly_rust/queries/get_app_compact_schema.graphql",
    query_path = "src/fly_rust/queries/get_app_compact.graphql",
    response_derives = "Debug"
)]
struct GetAppCompact;
#[instrument(err)]
pub async fn get_app_compact(
    request_builder_graphql: &RequestBuilderGraphql,
    app_name: String,
) -> RdrResult<Option<get_app_compact::ResponseData>> {
    let variables = get_app_compact::Variables { app_name };
    let request_body = GetAppCompact::build_query(variables);
    let response = request_builder_graphql
        .query()
        .json(&request_body)
        .send()
        .await?;
    let response_body: Response<get_app_compact::ResponseData> = response.json().await?;
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

/// Get App Basic
#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "src/fly_rust/queries/get_app_basic_schema.graphql",
    query_path = "src/fly_rust/queries/get_app_basic.graphql",
    response_derives = "Debug"
)]
struct GetAppBasic;
#[instrument(err)]
pub async fn get_app_basic(
    request_builder_graphql: &RequestBuilderGraphql,
    app_name: String,
) -> RdrResult<Option<get_app_basic::ResponseData>> {
    let variables = get_app_basic::Variables { app_name };
    let request_body = GetAppBasic::build_query(variables);
    let response = request_builder_graphql
        .query()
        .json(&request_body)
        .send()
        .await?;
    let response_body: Response<get_app_basic::ResponseData> = response.json().await?;
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

/// Delete App
#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "src/fly_rust/queries/delete_app_schema.graphql",
    query_path = "src/fly_rust/queries/delete_app.graphql",
    response_derives = "Debug"
)]
struct DeleteApp;
#[instrument(err)]
pub async fn delete_app(
    request_builder_graphql: &RequestBuilderGraphql,
    app_name: String,
) -> RdrResult<Option<delete_app::ResponseData>> {
    let variables = delete_app::Variables { app_id: app_name };
    let request_body = DeleteApp::build_query(variables);
    let response = request_builder_graphql
        .query()
        .json(&request_body)
        .send()
        .await?;
    let response_body: Response<delete_app::ResponseData> = response.json().await?;
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
