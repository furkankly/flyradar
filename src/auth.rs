use color_eyre::eyre;
use tracing::debug;

use crate::config::helpers::get_config_file_path;
use crate::config::{self};
use crate::state::RdrResult;

pub async fn read_access_token() -> RdrResult<String> {
    let config_file_path = get_config_file_path();

    match config_file_path {
        Ok(config_file_path) => match config::file::read_access_token(config_file_path).await {
            Ok(token) => {
                if token.is_empty() {
                    println!("Make sure to be authenticated to Fly.io to use flyradar. Try \"fly auth signup\" to create an
account, or \"fly auth login\" to log in to an existing account.");
                    Err(eyre::eyre!("Token is empty."))
                } else {
                    Ok(token)
                }
            }
            Err(err) => {
                debug!("Auth failed: {:#?}", err);
                println!("Make sure to be authenticated to Fly.io to use flyradar. Try \"fly auth signup\" to create an
account, or \"fly auth login\" to log in to an existing account.");
                Err(eyre::eyre!("Auth failed: {:#?}", err))
            }
        },
        Err(_) => {
            println!("Your fly.io config file is not found. Make sure to be authenticated to Fly.io to use flyradar. Try \"fly auth signup\" to create an
account, or \"fly auth login\" to log in to an existing account.");
            Err(eyre::eyre!("Config file not found."))
        }
    }
}
