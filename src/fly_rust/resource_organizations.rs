use backon::{ConstantBuilder, Retryable};
use color_eyre::eyre::eyre;
use graphql_client::{GraphQLQuery, Response};
use tracing::{info, instrument};

use super::request_builder::{find_err, RequestBuilderGraphql};
use crate::state::RdrResult;
use crate::transformations::ListOrganization;

#[derive(Clone, Debug, Default)]
pub struct OrganizationFilter {
    admin: bool,
}

impl OrganizationFilter {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn admin_only() -> Self {
        Self::new().admin(true)
    }

    pub fn admin(mut self, value: bool) -> Self {
        self.admin = value;
        self
    }

    pub fn is_admin_only(&self) -> bool {
        self.admin
    }
}

pub async fn get_all_organizations(
    request_builder_graphql: &RequestBuilderGraphql,
    filter: OrganizationFilter,
) -> RdrResult<Vec<ListOrganization>> {
    let mut all_orgs = vec![];
    if let Some(response) = get_organizations(request_builder_graphql, filter).await? {
        all_orgs.extend(
            response
                .organizations
                .nodes
                .iter()
                .map(|org| ListOrganization {
                    id: org.id.clone(),
                    name: org.name.clone(),
                    viewer_role: org.viewer_role.clone(),
                    slug: org.slug.clone(),
                    type_: org.type_.clone(),
                }),
        );
    }
    info!("List of organizations: {:#?}", all_orgs);
    Ok(all_orgs)
}

/// Get Organizations
#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "src/fly_rust/queries/get_organizations_schema.graphql",
    query_path = "src/fly_rust/queries/get_organizations.graphql",
    response_derives = "Debug"
)]
struct GetOrganizations;
#[instrument(err)]
pub async fn get_organizations(
    request_builder_graphql: &RequestBuilderGraphql,
    filter: OrganizationFilter,
) -> RdrResult<Option<get_organizations::ResponseData>> {
    let variables = get_organizations::Variables {
        admin: filter.admin,
    };
    let request_body = GetOrganizations::build_query(variables);

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
    let response_body: Response<get_organizations::ResponseData> =
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

/// Get Detailed Organization By Slug
#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "src/fly_rust/queries/get_detailed_organization_by_slug_schema.graphql",
    query_path = "src/fly_rust/queries/get_detailed_organization_by_slug.graphql",
    response_derives = "Debug"
)]
pub struct GetDetailedOrganizationBySlug;
#[instrument(err)]
pub async fn get_detailed_organization_by_slug(
    request_builder_graphql: &RequestBuilderGraphql,
    org_slug: String,
) -> RdrResult<Option<get_detailed_organization_by_slug::ResponseData>> {
    let variables = get_detailed_organization_by_slug::Variables { slug: org_slug };
    let request_body = GetDetailedOrganizationBySlug::build_query(variables);
    let response = request_builder_graphql
        .query()
        .json(&request_body)
        .send()
        .await?;
    let bytes = response.bytes().await?;
    let response_body: Response<get_detailed_organization_by_slug::ResponseData> =
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

/// Delete Organization
#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "src/fly_rust/queries/delete_organization_schema.graphql",
    query_path = "src/fly_rust/queries/delete_organization.graphql",
    response_derives = "Debug"
)]
pub struct DeleteOrganization;
#[instrument(err)]
pub async fn delete_organization(
    request_builder_graphql: &RequestBuilderGraphql,
    organization_id: String,
) -> RdrResult<Option<delete_organization::ResponseData>> {
    let variables = delete_organization::Variables {
        input: delete_organization::DeleteOrganizationInput { organization_id },
    };
    let request_body = DeleteOrganization::build_query(variables);
    let response = request_builder_graphql
        .query()
        .json(&request_body)
        .send()
        .await?;
    let bytes = response.bytes().await?;
    let response_body: Response<delete_organization::ResponseData> =
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

/// Create Organization Invite
#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "src/fly_rust/queries/create_organization_invite_schema.graphql",
    query_path = "src/fly_rust/queries/create_organization_invite.graphql",
    response_derives = "Debug"
)]
pub struct CreateOrganizationInvite;
#[instrument(err)]
pub async fn create_organization_invite(
    request_builder_graphql: &RequestBuilderGraphql,
    organization_id: String,
    email: String,
) -> RdrResult<Option<create_organization_invite::ResponseData>> {
    let variables = create_organization_invite::Variables {
        input: create_organization_invite::CreateOrganizationInvitationInput {
            organization_id,
            email,
        },
    };
    let request_body = CreateOrganizationInvite::build_query(variables);
    let response = request_builder_graphql
        .query()
        .json(&request_body)
        .send()
        .await?;
    let bytes = response.bytes().await?;
    let response_body: Response<create_organization_invite::ResponseData> =
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

/// Delete Organization Membership
#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "src/fly_rust/queries/delete_organization_membership_schema.graphql",
    query_path = "src/fly_rust/queries/delete_organization_membership.graphql",
    response_derives = "Debug"
)]
pub struct DeleteOrganizationMembership;
#[instrument(err)]
pub async fn delete_organization_membership(
    request_builder_graphql: &RequestBuilderGraphql,
    organization_id: String,
    user_id: String,
) -> RdrResult<Option<delete_organization_membership::ResponseData>> {
    let variables = delete_organization_membership::Variables {
        input: delete_organization_membership::DeleteOrganizationMembershipInput {
            organization_id,
            user_id,
        },
    };
    let request_body = DeleteOrganizationMembership::build_query(variables);
    let response = request_builder_graphql
        .query()
        .json(&request_body)
        .send()
        .await?;
    let bytes = response.bytes().await?;
    let response_body: Response<delete_organization_membership::ResponseData> =
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
