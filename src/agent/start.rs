use std::fs::{self};
use std::path::PathBuf;
use std::time::{
    Duration, {self},
};

use color_eyre::eyre::{
    Context, {self},
};
use fslock::LockFile;
use tempfile::NamedTempFile;
use tokio::process::Command;
use tracing::{error, info};

use super::client::{default_client, Client};
use crate::agent::set_sys_proc_attributes;
use crate::auth::read_access_token;
use crate::config::helpers::get_config_directory;
use crate::state::RdrResult;

const NO_UPDATE_CHECK: &str = "FLY_NO_UPDATE_CHECK";
const API_TOKEN_ENV: &str = "FLY_API_TOKEN";

/// Creates and configures a log file for the daemon
async fn create_log_file() -> RdrResult<PathBuf> {
    let dir = setup_log_directory().await?;

    let temp_file = NamedTempFile::new_in(dir).wrap_err("failed creating log file")?;

    let path = temp_file.path().to_owned();
    temp_file
        .persist(&path)
        .wrap_err("failed persisting log file")?;

    Ok(path)
}

/// Sets up and cleans the log directory
async fn setup_log_directory() -> RdrResult<PathBuf> {
    let dir = get_config_directory()
        .expect("failed to get config directory")
        .join("agent-logs");
    fs::create_dir_all(&dir)
        .wrap_err_with(|| format!("failed creating agent log directory at {}", dir.display()))?;

    // Clean old logs (older than 1 day)
    let entries = fs::read_dir(&dir).wrap_err("failed reading agent log directory entries")?;

    let cutoff = time::SystemTime::now()
        .checked_sub(Duration::from_secs(24 * 60 * 60))
        .expect("time calculation error");

    entries
        .filter_map(Result::ok)
        .filter_map(|entry| Some((entry.path(), entry.metadata().ok()?)))
        .filter(|(_, metadata)| metadata.is_file())
        .filter_map(|(path, metadata)| Some((path, metadata.modified().ok()?)))
        .filter(|(_, modified)| *modified < cutoff)
        .for_each(|(path, _)| {
            let _ = fs::remove_file(path);
        });

    Ok(dir)
}

/// Gets a lock to ensure only one process starts the daemon
fn lock() -> RdrResult<LockFile> {
    let lock_path = get_config_directory()
        .expect("failed to get config directory")
        .join("flyctl.agent.start.lock");
    let mut lock = LockFile::open(&lock_path).wrap_err("failed to open lock file")?;

    match lock.try_lock() {
        Ok(_) => Ok(lock),
        Err(e) => Err(eyre::eyre!(
            "another process is already starting the agent: {}",
            e
        )),
    }
}

/// Reads the contents of a log file, truncating if too large
fn read_log_file(path: &PathBuf) -> String {
    match fs::read_to_string(path) {
        Ok(content) => {
            const LIMIT: usize = 10 * 1024; // 10KB
            if content.len() > LIMIT {
                content[..LIMIT].to_string()
            } else {
                content
            }
        }
        Err(_) => String::new(),
    }
}

/// Waits for the client to become available
async fn wait_for_client() -> RdrResult<Client> {
    let mut interval = tokio::time::interval(Duration::from_millis(50));
    let start = std::time::Instant::now();
    let timeout = Duration::from_secs(5);

    while start.elapsed() < timeout {
        interval.tick().await;

        if let Ok(client) = default_client().await {
            return Ok(client);
        }
    }

    info!("timeout waiting for agent to start");
    Err(eyre::eyre!("timeout waiting for agent to start"))
}

/// Starts the daemon process
pub async fn start_daemon() -> RdrResult<Client> {
    // Get lock to ensure single starter
    let _lock = lock()?;

    // Create log file
    let log_file = create_log_file().await?;

    // Prepare command
    let mut command = Command::new("flyctl");
    command.args(["agent", "run"]);
    command.arg(&log_file);

    set_sys_proc_attributes::set_process_attributes(&mut command);

    // Setup environment
    command.env(NO_UPDATE_CHECK, "1");
    if let Ok(token) = read_access_token().await {
        command.env(API_TOKEN_ENV, token);
    }

    // Start process asynchronously
    let pid = {
        let child = command.spawn().wrap_err("failed starting agent process")?;
        child
            .id()
            .ok_or_else(|| eyre::eyre!("could not get process id"))?
    };

    info!(
        "started agent process (pid: {}, log: {})",
        pid,
        log_file.display()
    );

    // Wait for client to become available
    match wait_for_client().await {
        Ok(client) => Ok(client),
        Err(err) => {
            let log = read_log_file(&log_file);

            let error_msg = format!(
                "The agent failed to start with the following error log:\n\n{}\n\nA copy of this log has been saved at {}",
                log,
                log_file.display()
            );

            error!("{}", error_msg);
            Err(err.wrap_err(error_msg))
        }
    }
}
