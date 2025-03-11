use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use apps::restart::AppRestartParams;
use logs::LogsResources;
use machines::kill::KillMachineInput;
use reqwest::Client;
use tokio::sync::oneshot;
use tokio::time::sleep;
use tokio_util::sync::CancellationToken;

use crate::config::{FullConfig, DEFAULT_API_BASE_URL, DEFAULT_FLAPS_BASE_URL};
use crate::fly_rust::machine_types::{RemoveMachineInput, RestartMachineInput, StopMachineInput};
use crate::fly_rust::request_builder::{
    RequestBuilderFly, RequestBuilderGraphql, RequestBuilderMachines, {self},
};
use crate::fly_rust::volume_types::RemoveVolumeInput;
use crate::logs::LogOptions;
use crate::state::{PopupType, RdrPopup, SharedState};
use crate::widgets::log_viewer::dump_logs;

pub mod apps;
mod lease;
pub mod logs;
pub mod machines;
pub mod secrets;
pub mod select_many_machines;
pub mod volumes;
mod wait;

#[derive(Debug)]
pub enum IoEvent {
    ListApps,
    OpenApp(String),
    ViewAppReleases(String),
    ViewAppServices(String),
    RestartApp(String, AppRestartParams),
    DestroyApp(String),
    ListMachines(String),
    RestartMachines(String, Vec<String>, RestartMachineInput),
    StartMachines(String, Vec<String>),
    StopMachines(String, Vec<String>, StopMachineInput),
    KillMachine(String, KillMachineInput),
    SuspendMachines(String, Vec<String>),
    DestroyMachine(String, RemoveMachineInput),
    CordonMachines(String, Vec<String>),
    UncordonMachines(String, Vec<String>),
    StreamLogs(LogOptions),
    DumpLogs(PathBuf),
    StopLogs,
    ListVolumes(String),
    DestroyVolume(String, RemoveVolumeInput),
    ListSecrets(String),
    UnsetSecrets(String, Vec<String>),
}

#[derive(Clone)]
pub struct Ops {
    pub request_builder_machines: RequestBuilderMachines,
    pub request_builder_graphql: RequestBuilderGraphql,
    request_builder_fly: RequestBuilderFly,
    logs_resources: Arc<Mutex<LogsResources>>,
    shared_state: Arc<Mutex<SharedState>>,
}

impl Ops {
    pub fn new(config: FullConfig, shared_state: Arc<Mutex<SharedState>>) -> Self {
        //INFO: Fly.io apis close the connection with a keep-alive timeout value lower than hyper's default 90sec, hence we need this.
        let http_client = Client::builder()
            .pool_idle_timeout(Duration::from_secs(40))
            .build()
            .unwrap();
        Ops {
            request_builder_machines: request_builder::RequestBuilderMachines::new(
                http_client.clone(),
                DEFAULT_FLAPS_BASE_URL.to_string(),
                config.token_config.access_token.clone(),
            ),
            request_builder_graphql: request_builder::RequestBuilderGraphql::new(
                http_client.clone(),
                format!("{DEFAULT_API_BASE_URL}/graphql"),
                config.token_config.access_token.clone(),
            ),
            // Used only for polling vm logs
            request_builder_fly: request_builder::RequestBuilderFly::new(
                http_client,
                format!("{DEFAULT_API_BASE_URL}/api"),
                config.token_config.access_token,
            ),
            logs_resources: Arc::new(Mutex::new(LogsResources {
                cancellation_token_nats: CancellationToken::new(),
                polling_handle: None,
                nats: None,
            })),
            shared_state,
        }
    }

    async fn cleanup_logs_resources(&mut self) {
        let (polling_handle, nats) = {
            let mut resources = self.logs_resources.lock().unwrap();
            (resources.polling_handle.take(), resources.nats.take())
        };

        if let Some(polling_handle) = polling_handle {
            polling_handle.abort();
        }
        if let Some(nats) = nats {
            let _ = nats.nc.drain().await;
        }
    }

    pub async fn handle_ops_event(&mut self, io_event: IoEvent) {
        match io_event {
            IoEvent::ListApps => {
                if let Err(err) = apps::list::list(self).await {
                    let mut shared_state_guard = self.shared_state.lock().unwrap();
                    shared_state_guard.popup =
                        Some(RdrPopup::new(PopupType::ErrorPopup, err.to_string()));
                }
            }
            IoEvent::OpenApp(app_name) => {
                if let Err(err) = apps::open::open(self, app_name).await {
                    let mut shared_state_guard = self.shared_state.lock().unwrap();
                    shared_state_guard.popup =
                        Some(RdrPopup::new(PopupType::ErrorPopup, err.to_string()));
                }
            }
            IoEvent::ViewAppReleases(app_name) => {
                if let Err(err) = apps::releases::releases(self, app_name, 25).await {
                    let mut shared_state_guard = self.shared_state.lock().unwrap();
                    shared_state_guard.popup =
                        Some(RdrPopup::new(PopupType::ErrorPopup, err.to_string()));
                }
            }
            IoEvent::ViewAppServices(app_name) => {
                if let Err(err) = apps::services::services(self, app_name).await {
                    let mut shared_state_guard = self.shared_state.lock().unwrap();
                    shared_state_guard.popup =
                        Some(RdrPopup::new(PopupType::ErrorPopup, err.to_string()));
                }
            }
            IoEvent::RestartApp(app_name, restart_params) => {
                if let Err(err) = apps::restart::restart(self, &app_name, restart_params).await {
                    let mut shared_state_guard = self.shared_state.lock().unwrap();
                    shared_state_guard.popup =
                        Some(RdrPopup::new(PopupType::ErrorPopup, err.to_string()));
                }
            }
            IoEvent::DestroyApp(app_name) => {
                if let Err(err) = apps::destroy::destroy(self, app_name).await {
                    let mut shared_state_guard = self.shared_state.lock().unwrap();
                    shared_state_guard.popup =
                        Some(RdrPopup::new(PopupType::ErrorPopup, err.to_string()));
                } else if let Err(err) = apps::list::list(self).await {
                    let mut shared_state_guard = self.shared_state.lock().unwrap();
                    shared_state_guard.popup =
                        Some(RdrPopup::new(PopupType::ErrorPopup, err.to_string()));
                }
            }
            IoEvent::ListMachines(app_name) => {
                if let Err(err) = machines::list::list(self, &app_name).await {
                    let mut shared_state_guard = self.shared_state.lock().unwrap();
                    shared_state_guard.popup =
                        Some(RdrPopup::new(PopupType::ErrorPopup, err.to_string()));
                }
            }
            IoEvent::RestartMachines(app_name, machines, restart_params) => {
                if let Err(err) =
                    machines::restart::restart(self, &app_name, machines, restart_params).await
                {
                    let mut shared_state_guard = self.shared_state.lock().unwrap();
                    shared_state_guard.popup =
                        Some(RdrPopup::new(PopupType::ErrorPopup, err.to_string()));
                }
            }
            IoEvent::StartMachines(app_name, machines) => {
                if let Err(err) = machines::start::start(self, &app_name, machines).await {
                    let mut shared_state_guard = self.shared_state.lock().unwrap();
                    shared_state_guard.popup =
                        Some(RdrPopup::new(PopupType::ErrorPopup, err.to_string()));
                } else if let Err(err) = machines::list::list(self, &app_name).await {
                    let mut shared_state_guard = self.shared_state.lock().unwrap();
                    shared_state_guard.popup =
                        Some(RdrPopup::new(PopupType::ErrorPopup, err.to_string()));
                }
            }
            IoEvent::StopMachines(app_name, machines, stop_params) => {
                if let Err(err) = machines::stop::stop(self, &app_name, machines, stop_params).await
                {
                    let mut shared_state_guard = self.shared_state.lock().unwrap();
                    shared_state_guard.popup =
                        Some(RdrPopup::new(PopupType::ErrorPopup, err.to_string()));
                } else if let Err(err) = machines::list::list(self, &app_name).await {
                    let mut shared_state_guard = self.shared_state.lock().unwrap();
                    shared_state_guard.popup =
                        Some(RdrPopup::new(PopupType::ErrorPopup, err.to_string()));
                }
            }
            IoEvent::KillMachine(app_name, kill_params) => {
                if let Err(err) = machines::kill::kill(self, &app_name, kill_params).await {
                    let mut shared_state_guard = self.shared_state.lock().unwrap();
                    shared_state_guard.popup =
                        Some(RdrPopup::new(PopupType::ErrorPopup, err.to_string()));
                } else {
                    let mut shared_state_guard = self.shared_state.lock().unwrap();
                    shared_state_guard.popup = Some(RdrPopup::new(
                        PopupType::InfoPopup,
                        String::from("Kill signal has been sent."),
                    ));
                }
                if let Err(err) = machines::list::list(self, &app_name).await {
                    let mut shared_state_guard = self.shared_state.lock().unwrap();
                    shared_state_guard.popup =
                        Some(RdrPopup::new(PopupType::ErrorPopup, err.to_string()));
                }
            }
            IoEvent::SuspendMachines(app_name, machines) => {
                if let Err(err) = machines::suspend::suspend(self, &app_name, machines).await {
                    let mut shared_state_guard = self.shared_state.lock().unwrap();
                    shared_state_guard.popup =
                        Some(RdrPopup::new(PopupType::ErrorPopup, err.to_string()));
                } else if let Err(err) = machines::list::list(self, &app_name).await {
                    let mut shared_state_guard = self.shared_state.lock().unwrap();
                    shared_state_guard.popup =
                        Some(RdrPopup::new(PopupType::ErrorPopup, err.to_string()));
                }
            }
            IoEvent::DestroyMachine(app_name, destroy_params) => {
                if let Err(err) = machines::destroy::destroy(self, &app_name, destroy_params).await
                {
                    let mut shared_state_guard = self.shared_state.lock().unwrap();
                    shared_state_guard.popup =
                        Some(RdrPopup::new(PopupType::ErrorPopup, err.to_string()));
                } else if let Err(err) = machines::list::list(self, &app_name).await {
                    let mut shared_state_guard = self.shared_state.lock().unwrap();
                    shared_state_guard.popup =
                        Some(RdrPopup::new(PopupType::ErrorPopup, err.to_string()));
                }
            }
            IoEvent::CordonMachines(app_name, machines) => {
                if let Err(err) = machines::cordon::cordon(self, &app_name, machines).await {
                    let mut shared_state_guard = self.shared_state.lock().unwrap();
                    shared_state_guard.popup =
                        Some(RdrPopup::new(PopupType::ErrorPopup, err.to_string()));
                } else {
                    let mut shared_state_guard = self.shared_state.lock().unwrap();
                    shared_state_guard.popup = Some(RdrPopup::new(
                        PopupType::InfoPopup,
                        format!(
                            "Successfully cordoned the selected machines for {}.",
                            app_name
                        ),
                    ));
                }
                if let Err(err) = machines::list::list(self, &app_name).await {
                    let mut shared_state_guard = self.shared_state.lock().unwrap();
                    shared_state_guard.popup =
                        Some(RdrPopup::new(PopupType::ErrorPopup, err.to_string()));
                }
            }
            IoEvent::UncordonMachines(app_name, machines) => {
                if let Err(err) = machines::uncordon::uncordon(self, &app_name, machines).await {
                    let mut shared_state_guard = self.shared_state.lock().unwrap();
                    shared_state_guard.popup =
                        Some(RdrPopup::new(PopupType::ErrorPopup, err.to_string()));
                } else {
                    let mut shared_state_guard = self.shared_state.lock().unwrap();
                    shared_state_guard.popup = Some(RdrPopup::new(
                        PopupType::InfoPopup,
                        format!(
                            "Successfully uncordoned the selected machines for {}.",
                            app_name
                        ),
                    ));
                }
                if let Err(err) = machines::list::list(self, &app_name).await {
                    let mut shared_state_guard = self.shared_state.lock().unwrap();
                    shared_state_guard.popup =
                        Some(RdrPopup::new(PopupType::ErrorPopup, err.to_string()));
                }
            }
            IoEvent::StreamLogs(opts) => {
                let cancellation_token_nats = {
                    let mut resources = self.logs_resources.lock().unwrap();
                    resources.cancellation_token_nats = CancellationToken::new();
                    resources.cancellation_token_nats.clone()
                };
                if let Err(err) = logs::logs(self, &opts, cancellation_token_nats).await {
                    self.cleanup_logs_resources().await;
                    let mut shared_state_guard = self.shared_state.lock().unwrap();
                    shared_state_guard.popup =
                        Some(RdrPopup::new(PopupType::ErrorPopup, err.to_string()));
                }
            }
            IoEvent::DumpLogs(file_path) => {
                if let Err(err) = dump_logs(&file_path).await {
                    let mut shared_state_guard = self.shared_state.lock().unwrap();
                    shared_state_guard.popup =
                        Some(RdrPopup::new(PopupType::ErrorPopup, err.to_string()));
                } else {
                    let mut shared_state_guard = self.shared_state.lock().unwrap();
                    shared_state_guard.popup = Some(RdrPopup::new(
                        PopupType::InfoPopup,
                        format!(
                            "Successfully dumped the logs to {}.",
                            file_path.to_string_lossy()
                        ),
                    ));
                }
            }
            IoEvent::StopLogs => {
                self.logs_resources
                    .lock()
                    .unwrap()
                    .cancellation_token_nats
                    .cancel();
                self.cleanup_logs_resources().await;
            }
            IoEvent::ListVolumes(app_name) => {
                if let Err(err) = volumes::list::list(self, &app_name).await {
                    let mut shared_state_guard = self.shared_state.lock().unwrap();
                    shared_state_guard.popup =
                        Some(RdrPopup::new(PopupType::ErrorPopup, err.to_string()));
                }
            }
            IoEvent::DestroyVolume(app_name, destroy_params) => {
                if let Err(err) = volumes::destroy::destroy(self, &app_name, destroy_params).await {
                    let mut shared_state_guard = self.shared_state.lock().unwrap();
                    shared_state_guard.popup =
                        Some(RdrPopup::new(PopupType::ErrorPopup, err.to_string()));
                } else if let Err(err) = volumes::list::list(self, &app_name).await {
                    let mut shared_state_guard = self.shared_state.lock().unwrap();
                    shared_state_guard.popup =
                        Some(RdrPopup::new(PopupType::ErrorPopup, err.to_string()));
                }
            }
            IoEvent::ListSecrets(app_name) => {
                if let Err(err) = secrets::list::list(self, &app_name).await {
                    let mut shared_state_guard = self.shared_state.lock().unwrap();
                    shared_state_guard.popup =
                        Some(RdrPopup::new(PopupType::ErrorPopup, err.to_string()));
                }
            }
            IoEvent::UnsetSecrets(app_name, keys) => {
                if let Err(err) = secrets::unset::unset(self, &app_name, keys).await {
                    let mut shared_state_guard = self.shared_state.lock().unwrap();
                    shared_state_guard.popup =
                        Some(RdrPopup::new(PopupType::ErrorPopup, err.to_string()));
                } else if let Err(err) = secrets::list::list(self, &app_name).await {
                    let mut shared_state_guard = self.shared_state.lock().unwrap();
                    shared_state_guard.popup =
                        Some(RdrPopup::new(PopupType::ErrorPopup, err.to_string()));
                }
            }
        }
    }

    /// INFO: Always assign the return value to a var to show the feedback.
    /// Drop the returned sender to cancel the feedback.
    pub fn show_delayed_feedback(&self, message: String, delay: Duration) -> oneshot::Sender<()> {
        let (feedback_tx, feedback_rx) = oneshot::channel::<()>();
        let shared_state_clone = Arc::clone(&self.shared_state);
        tokio::spawn(async move {
            tokio::select! {
                _ = sleep(delay) => {
                    let mut shared_state_guard = shared_state_clone.lock().unwrap();
                    shared_state_guard.popup = Some(RdrPopup::new(PopupType::InfoPopup, message));
                }
                _ = feedback_rx => {
                    // Feedback cancelled, don't show popup
                }
            }
        });

        feedback_tx
    }
}
