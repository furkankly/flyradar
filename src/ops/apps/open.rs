use color_eyre::eyre::eyre;
use reqwest::Url;

use crate::fly_rust::resource_apps::get_app_compact;
use crate::ops::Ops;
use crate::state::RdrResult;

pub async fn open(ops: &Ops, app_name: String) -> RdrResult<()> {
    let response = get_app_compact(&ops.request_builder_graphql, app_name).await?;
    if let Some(response) = response {
        let url = Url::parse(&format!("https://{}", &response.appcompact.hostname))?;
        webbrowser::open(url.as_str()).map_err(|_err| eyre!("Could not open the application."))?;
        return Ok(());
    }
    Ok(())
}
