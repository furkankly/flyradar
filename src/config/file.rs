use std::collections::HashMap;
use std::path::PathBuf;

use fslock::LockFile;
use serde::Deserialize;
use tokio::fs::{File, OpenOptions};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

use super::{FullConfig, TokenConfig, WIREGUARD_STATE_FILE_KEY};
use crate::config::helpers::get_config_directory;
use crate::state::RdrResult;
use crate::wireguard::WireGuardStates;

/// Read a value from the config file.
async fn read<T: for<'de> Deserialize<'de>>(path: impl Into<PathBuf>) -> RdrResult<T> {
    let path = path.into();
    let lock_path = lock_path()?;
    let mut lock = LockFile::open(&lock_path)?;
    lock.lock()?;

    let mut file = File::open(&path).await?;
    let mut contents = String::new();
    file.read_to_string(&mut contents).await?;
    let result = serde_yaml::from_str(&contents)?;

    lock.unlock()?;
    Ok(result)
}

/// Write values to the config file.
async fn set(
    path: impl Into<PathBuf>,
    values: HashMap<String, serde_json::Value>,
) -> RdrResult<()> {
    let path = path.into();
    let lock_path = lock_path()?;
    let mut lock = LockFile::open(&lock_path)?;
    lock.lock()?;

    // Read existing config or create new
    let mut config: HashMap<String, serde_json::Value> = match File::open(&path).await {
        Ok(mut file) => {
            let mut contents = String::new();
            file.read_to_string(&mut contents).await?;
            serde_yaml::from_str(&contents)?
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => HashMap::new(),
        Err(e) => return Err(e.into()),
    };

    // Update with new values
    config.extend(values);

    // Write back to file
    let yaml = serde_yaml::to_string(&config)?;
    let mut open_options = OpenOptions::new();
    open_options.write(true);
    open_options.create(true);
    open_options.truncate(true);

    #[cfg(unix)]
    {
        open_options.mode(0o600);
    }

    let mut file = open_options.open(&path).await?;
    file.write_all(yaml.as_bytes()).await?;
    file.flush().await?;

    lock.unlock()?;
    Ok(())
}

pub async fn read_wg_state(path: impl Into<PathBuf>) -> RdrResult<Option<WireGuardStates>> {
    let config: FullConfig = read(path).await?;
    Ok(config.wire_guard_state)
}

pub async fn set_wg_state(path: impl Into<PathBuf>, states: WireGuardStates) -> RdrResult<()> {
    let mut values = HashMap::new();
    values.insert(
        WIREGUARD_STATE_FILE_KEY.to_string(),
        serde_json::to_value(states)?,
    );
    set(path, values).await
}

pub async fn read_access_token(path: impl Into<PathBuf>) -> RdrResult<String> {
    let config: TokenConfig = read(path).await?;
    Ok(config.access_token)
}

pub async fn set_access_token(path: impl Into<PathBuf>, token: String) -> RdrResult<()> {
    let mut values = HashMap::new();
    values.insert("access_token".to_string(), serde_json::to_value(token)?);
    set(path, values).await
}

pub fn lock_path() -> RdrResult<String> {
    let config_dir = get_config_directory()?;
    let config_dir = config_dir.to_string_lossy();
    Ok(format!("{config_dir}/flyctl.config.lock"))
}
