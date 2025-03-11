use std::env;
use std::path::PathBuf;

use color_eyre::eyre::OptionExt;
use directories::UserDirs;

use crate::state::RdrResult;

pub fn get_config_directory() -> RdrResult<PathBuf> {
    // First check if FLY_CONFIG_DIR is set
    if let Ok(value) = env::var("FLY_CONFIG_DIR") {
        return Ok(PathBuf::from(value));
    }

    // If not, use $HOME/.fly
    let home_dir = UserDirs::new()
        .ok_or_eyre("Could not determine home directory.")?
        .home_dir()
        .to_path_buf();

    Ok(home_dir.join(".fly"))
}

pub fn get_config_file_path() -> RdrResult<PathBuf> {
    get_config_directory().map(|config_dir| config_dir.join("config.yml"))
}
