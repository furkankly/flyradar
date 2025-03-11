use crate::fly_rust::resource_secrets::unset_secrets;
use crate::ops::Ops;
use crate::state::RdrResult;

pub async fn unset(ops: &Ops, app_name: &str, keys: Vec<String>) -> RdrResult<()> {
    unset_secrets(&ops.request_builder_graphql, app_name.to_string(), keys).await?;
    Ok(())
}
