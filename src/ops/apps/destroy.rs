use crate::fly_rust::resource_apps::delete_app;
use crate::ops::Ops;
use crate::state::RdrResult;

//TODO: Find the tigris statics bucket for the given app and org and delete the add-on.
pub async fn destroy(ops: &Ops, app_name: String) -> RdrResult<()> {
    delete_app(&ops.request_builder_graphql, app_name).await?;
    Ok(())
}
