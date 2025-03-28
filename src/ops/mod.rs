use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use apps::restart::AppRestartParams;
use logs::LogsResources;
use machines::kill::KillMachineInput;
use reqwest::Client;
use tokio::sync::mpsc::Sender;
use tokio::sync::oneshot;
use tokio::time::sleep;
use tokio_util::sync::CancellationToken;

use crate::config::{FullConfig, DEFAULT_API_BASE_URL, DEFAULT_FLAPS_BASE_URL};
use crate::fly_rust::machine_types::{RemoveMachineInput, RestartMachineInput, StopMachineInput};
use crate::fly_rust::request_builder::{
    RequestBuilderFly, RequestBuilderGraphql, RequestBuilderMachines, {self},
};
use crate::fly_rust::resource_organizations::OrganizationFilter;
use crate::fly_rust::volume_types::RemoveVolumeInput;
use crate::logs::LogOptions;
use crate::state::PopupType;
use crate::widgets::log_viewer::dump_logs;

pub mod apps;
mod lease;
pub mod logs;
pub mod machines;
pub mod organizations;
pub mod secrets;
pub mod select_many_machines;
pub mod volumes;
mod wait;

#[derive(Debug)]
pub enum IoReqEvent {
    ListOrganizations {
        filter: OrganizationFilter,
    },
    ListApps {
        org_slug: String,
    },
    OpenApp {
        app_name: String,
    },
    ViewAppReleases {
        app_name: String,
    },
    ViewAppServices {
        app_name: String,
    },
    RestartApp {
        app_name: String,
        params: AppRestartParams,
        org_slug: String,
    },
    DestroyApp {
        app_name: String,
        org_slug: String,
    },
    ListMachines {
        app_name: String,
    },
    RestartMachines {
        app_name: String,
        machines: Vec<String>,
        params: RestartMachineInput,
    },
    StartMachines {
        app_name: String,
        machines: Vec<String>,
    },
    StopMachines {
        app_name: String,
        machines: Vec<String>,
        params: StopMachineInput,
    },
    KillMachine {
        app_name: String,
        params: KillMachineInput,
    },
    SuspendMachines {
        app_name: String,
        machines: Vec<String>,
    },
    DestroyMachine {
        app_name: String,
        params: RemoveMachineInput,
    },
    CordonMachines {
        app_name: String,
        machines: Vec<String>,
    },
    UncordonMachines {
        app_name: String,
        machines: Vec<String>,
    },
    StreamLogs {
        opts: LogOptions,
    },
    DumpLogs {
        file_path: PathBuf,
    },
    StopLogs,
    ListVolumes {
        app_name: String,
    },
    DestroyVolume {
        app_name: String,
        params: RemoveVolumeInput,
    },
    ListSecrets {
        app_name: String,
    },
    UnsetSecrets {
        app_name: String,
        keys: Vec<String>,
    },
}

#[derive(Debug)]
pub enum IoRespEvent {
    Organizations {
        list: Vec<Vec<String>>,
    },
    Apps {
        list: Vec<Vec<String>>,
    },
    Machines {
        list: Vec<Vec<String>>,
    },
    Volumes {
        list: Vec<Vec<String>>,
    },
    Secrets {
        list: Vec<Vec<String>>,
    },
    AppReleases {
        list: Vec<Vec<String>>,
    },
    AppServices {
        list: Vec<Vec<String>>,
    },
    SetPopup {
        popup_type: PopupType,
        message: String,
    },
}

#[derive(Clone)]
pub struct Ops {
    pub request_builder_machines: RequestBuilderMachines,
    pub request_builder_graphql: RequestBuilderGraphql,
    request_builder_fly: RequestBuilderFly,
    io_resp_tx: Sender<IoRespEvent>,
    logs_resources: Arc<Mutex<LogsResources>>,
}

impl Ops {
    pub fn new(config: FullConfig, io_resp_tx: Sender<IoRespEvent>) -> Self {
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
            io_resp_tx,
            logs_resources: Arc::new(Mutex::new(LogsResources {
                cancellation_token_nats: CancellationToken::new(),
                polling_handle: None,
                nats: None,
            })),
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

    pub async fn handle_io_req(&mut self, io_event: IoReqEvent) {
        match io_event {
            IoReqEvent::ListOrganizations { filter } => {
                if let Err(err) = organizations::list::list(self, filter).await {
                    let _ = self
                        .io_resp_tx
                        .send(IoRespEvent::SetPopup {
                            popup_type: PopupType::ErrorPopup,
                            message: err.to_string(),
                        })
                        .await;
                }
            }
            IoReqEvent::ListApps { org_slug } => {
                if let Err(err) = apps::list::list(self, org_slug).await {
                    let _ = self
                        .io_resp_tx
                        .send(IoRespEvent::SetPopup {
                            popup_type: PopupType::ErrorPopup,
                            message: err.to_string(),
                        })
                        .await;
                }
            }
            IoReqEvent::OpenApp { app_name } => {
                if let Err(err) = apps::open::open(self, app_name).await {
                    let _ = self
                        .io_resp_tx
                        .send(IoRespEvent::SetPopup {
                            popup_type: PopupType::ErrorPopup,
                            message: err.to_string(),
                        })
                        .await;
                }
            }
            IoReqEvent::ViewAppReleases { app_name } => {
                if let Err(err) = apps::releases::releases(self, app_name, 25).await {
                    let _ = self
                        .io_resp_tx
                        .send(IoRespEvent::SetPopup {
                            popup_type: PopupType::ErrorPopup,
                            message: err.to_string(),
                        })
                        .await;
                }
            }
            IoReqEvent::ViewAppServices { app_name } => {
                if let Err(err) = apps::services::services(self, app_name).await {
                    let _ = self
                        .io_resp_tx
                        .send(IoRespEvent::SetPopup {
                            popup_type: PopupType::ErrorPopup,
                            message: err.to_string(),
                        })
                        .await;
                }
            }
            IoReqEvent::RestartApp {
                app_name,
                params,
                org_slug,
            } => {
                if let Err(err) = apps::restart::restart(self, &app_name, params).await {
                    let _ = self
                        .io_resp_tx
                        .send(IoRespEvent::SetPopup {
                            popup_type: PopupType::ErrorPopup,
                            message: err.to_string(),
                        })
                        .await;
                } else if let Err(err) = apps::list::list(self, org_slug).await {
                    let _ = self
                        .io_resp_tx
                        .send(IoRespEvent::SetPopup {
                            popup_type: PopupType::ErrorPopup,
                            message: err.to_string(),
                        })
                        .await;
                }
            }
            IoReqEvent::DestroyApp { app_name, org_slug } => {
                if let Err(err) = apps::destroy::destroy(self, app_name).await {
                    let _ = self
                        .io_resp_tx
                        .send(IoRespEvent::SetPopup {
                            popup_type: PopupType::ErrorPopup,
                            message: err.to_string(),
                        })
                        .await;
                } else if let Err(err) = apps::list::list(self, org_slug).await {
                    let _ = self
                        .io_resp_tx
                        .send(IoRespEvent::SetPopup {
                            popup_type: PopupType::ErrorPopup,
                            message: err.to_string(),
                        })
                        .await;
                }
            }
            IoReqEvent::ListMachines { app_name } => {
                if let Err(err) = machines::list::list(self, &app_name).await {
                    let _ = self
                        .io_resp_tx
                        .send(IoRespEvent::SetPopup {
                            popup_type: PopupType::ErrorPopup,
                            message: err.to_string(),
                        })
                        .await;
                }
            }
            IoReqEvent::RestartMachines {
                app_name,
                machines,
                params,
            } => {
                if let Err(err) =
                    machines::restart::restart(self, &app_name, machines, params).await
                {
                    let _ = self
                        .io_resp_tx
                        .send(IoRespEvent::SetPopup {
                            popup_type: PopupType::ErrorPopup,
                            message: err.to_string(),
                        })
                        .await;
                }
            }
            IoReqEvent::StartMachines { app_name, machines } => {
                if let Err(err) = machines::start::start(self, &app_name, machines).await {
                    let _ = self
                        .io_resp_tx
                        .send(IoRespEvent::SetPopup {
                            popup_type: PopupType::ErrorPopup,
                            message: err.to_string(),
                        })
                        .await;
                } else if let Err(err) = machines::list::list(self, &app_name).await {
                    let _ = self
                        .io_resp_tx
                        .send(IoRespEvent::SetPopup {
                            popup_type: PopupType::ErrorPopup,
                            message: err.to_string(),
                        })
                        .await;
                }
            }
            IoReqEvent::StopMachines {
                app_name,
                machines,
                params,
            } => {
                if let Err(err) = machines::stop::stop(self, &app_name, machines, params).await {
                    let _ = self
                        .io_resp_tx
                        .send(IoRespEvent::SetPopup {
                            popup_type: PopupType::ErrorPopup,
                            message: err.to_string(),
                        })
                        .await;
                } else if let Err(err) = machines::list::list(self, &app_name).await {
                    let _ = self
                        .io_resp_tx
                        .send(IoRespEvent::SetPopup {
                            popup_type: PopupType::ErrorPopup,
                            message: err.to_string(),
                        })
                        .await;
                }
            }
            IoReqEvent::KillMachine { app_name, params } => {
                if let Err(err) = machines::kill::kill(self, &app_name, params).await {
                    let _ = self
                        .io_resp_tx
                        .send(IoRespEvent::SetPopup {
                            popup_type: PopupType::ErrorPopup,
                            message: err.to_string(),
                        })
                        .await;
                } else {
                    let _ = self
                        .io_resp_tx
                        .send(IoRespEvent::SetPopup {
                            popup_type: PopupType::InfoPopup,
                            message: String::from("Kill signal has been sent."),
                        })
                        .await;
                }
                if let Err(err) = machines::list::list(self, &app_name).await {
                    let _ = self
                        .io_resp_tx
                        .send(IoRespEvent::SetPopup {
                            popup_type: PopupType::ErrorPopup,
                            message: err.to_string(),
                        })
                        .await;
                }
            }
            IoReqEvent::SuspendMachines { app_name, machines } => {
                if let Err(err) = machines::suspend::suspend(self, &app_name, machines).await {
                    let _ = self
                        .io_resp_tx
                        .send(IoRespEvent::SetPopup {
                            popup_type: PopupType::ErrorPopup,
                            message: err.to_string(),
                        })
                        .await;
                } else if let Err(err) = machines::list::list(self, &app_name).await {
                    let _ = self
                        .io_resp_tx
                        .send(IoRespEvent::SetPopup {
                            popup_type: PopupType::ErrorPopup,
                            message: err.to_string(),
                        })
                        .await;
                }
            }
            IoReqEvent::DestroyMachine { app_name, params } => {
                if let Err(err) = machines::destroy::destroy(self, &app_name, params).await {
                    let _ = self
                        .io_resp_tx
                        .send(IoRespEvent::SetPopup {
                            popup_type: PopupType::ErrorPopup,
                            message: err.to_string(),
                        })
                        .await;
                } else if let Err(err) = machines::list::list(self, &app_name).await {
                    let _ = self
                        .io_resp_tx
                        .send(IoRespEvent::SetPopup {
                            popup_type: PopupType::ErrorPopup,
                            message: err.to_string(),
                        })
                        .await;
                }
            }
            IoReqEvent::CordonMachines { app_name, machines } => {
                if let Err(err) = machines::cordon::cordon(self, &app_name, machines).await {
                    let _ = self
                        .io_resp_tx
                        .send(IoRespEvent::SetPopup {
                            popup_type: PopupType::ErrorPopup,
                            message: err.to_string(),
                        })
                        .await;
                } else {
                    let _ = self
                        .io_resp_tx
                        .send(IoRespEvent::SetPopup {
                            popup_type: PopupType::InfoPopup,
                            message: format!(
                                "Successfully cordoned the selected machines for {}.",
                                app_name
                            ),
                        })
                        .await;
                }
                if let Err(err) = machines::list::list(self, &app_name).await {
                    let _ = self
                        .io_resp_tx
                        .send(IoRespEvent::SetPopup {
                            popup_type: PopupType::ErrorPopup,
                            message: err.to_string(),
                        })
                        .await;
                }
            }
            IoReqEvent::UncordonMachines { app_name, machines } => {
                if let Err(err) = machines::uncordon::uncordon(self, &app_name, machines).await {
                    let _ = self
                        .io_resp_tx
                        .send(IoRespEvent::SetPopup {
                            popup_type: PopupType::ErrorPopup,
                            message: err.to_string(),
                        })
                        .await;
                } else {
                    let _ = self
                        .io_resp_tx
                        .send(IoRespEvent::SetPopup {
                            popup_type: PopupType::InfoPopup,
                            message: format!(
                                "Successfully uncordoned the selected machines for {}.",
                                app_name
                            ),
                        })
                        .await;
                }
                if let Err(err) = machines::list::list(self, &app_name).await {
                    let _ = self
                        .io_resp_tx
                        .send(IoRespEvent::SetPopup {
                            popup_type: PopupType::ErrorPopup,
                            message: err.to_string(),
                        })
                        .await;
                }
            }
            IoReqEvent::StreamLogs { opts } => {
                let cancellation_token_nats = {
                    let mut resources = self.logs_resources.lock().unwrap();
                    resources.cancellation_token_nats = CancellationToken::new();
                    resources.cancellation_token_nats.clone()
                };
                if let Err(err) = logs::logs(self, &opts, cancellation_token_nats).await {
                    self.cleanup_logs_resources().await;
                    let _ = self
                        .io_resp_tx
                        .send(IoRespEvent::SetPopup {
                            popup_type: PopupType::ErrorPopup,
                            message: err.to_string(),
                        })
                        .await;
                }
            }
            IoReqEvent::DumpLogs { file_path } => {
                if let Err(err) = dump_logs(&file_path).await {
                    let _ = self
                        .io_resp_tx
                        .send(IoRespEvent::SetPopup {
                            popup_type: PopupType::ErrorPopup,
                            message: err.to_string(),
                        })
                        .await;
                } else {
                    let _ = self
                        .io_resp_tx
                        .send(IoRespEvent::SetPopup {
                            popup_type: PopupType::InfoPopup,
                            message: format!(
                                "Successfully dumped the logs to {}.",
                                file_path.to_string_lossy()
                            ),
                        })
                        .await;
                }
            }
            IoReqEvent::StopLogs => {
                self.logs_resources
                    .lock()
                    .unwrap()
                    .cancellation_token_nats
                    .cancel();
                self.cleanup_logs_resources().await;
            }
            IoReqEvent::ListVolumes { app_name } => {
                if let Err(err) = volumes::list::list(self, &app_name).await {
                    let _ = self
                        .io_resp_tx
                        .send(IoRespEvent::SetPopup {
                            popup_type: PopupType::ErrorPopup,
                            message: err.to_string(),
                        })
                        .await;
                }
            }
            IoReqEvent::DestroyVolume { app_name, params } => {
                if let Err(err) = volumes::destroy::destroy(self, &app_name, params).await {
                    let _ = self
                        .io_resp_tx
                        .send(IoRespEvent::SetPopup {
                            popup_type: PopupType::ErrorPopup,
                            message: err.to_string(),
                        })
                        .await;
                } else if let Err(err) = volumes::list::list(self, &app_name).await {
                    let _ = self
                        .io_resp_tx
                        .send(IoRespEvent::SetPopup {
                            popup_type: PopupType::ErrorPopup,
                            message: err.to_string(),
                        })
                        .await;
                }
            }
            IoReqEvent::ListSecrets { app_name } => {
                if let Err(err) = secrets::list::list(self, &app_name).await {
                    let _ = self
                        .io_resp_tx
                        .send(IoRespEvent::SetPopup {
                            popup_type: PopupType::ErrorPopup,
                            message: err.to_string(),
                        })
                        .await;
                }
            }
            IoReqEvent::UnsetSecrets { app_name, keys } => {
                if let Err(err) = secrets::unset::unset(self, &app_name, keys).await {
                    let _ = self
                        .io_resp_tx
                        .send(IoRespEvent::SetPopup {
                            popup_type: PopupType::ErrorPopup,
                            message: err.to_string(),
                        })
                        .await;
                } else if let Err(err) = secrets::list::list(self, &app_name).await {
                    let _ = self
                        .io_resp_tx
                        .send(IoRespEvent::SetPopup {
                            popup_type: PopupType::ErrorPopup,
                            message: err.to_string(),
                        })
                        .await;
                }
            }
        }
    }

    /// INFO: Always assign the return value to a var to show the feedback.
    /// Drop the returned sender to cancel the feedback.
    pub fn show_delayed_feedback(&self, message: String, delay: Duration) -> oneshot::Sender<()> {
        let (feedback_tx, feedback_rx) = oneshot::channel::<()>();
        let io_resp_tx = self.io_resp_tx.clone();
        tokio::spawn(async move {
            tokio::select! {
                _ = sleep(delay) => {
                    let _ = io_resp_tx.send(IoRespEvent::SetPopup {
                        popup_type: PopupType::InfoPopup,
                        message
                    }).await;
                }
                _ = feedback_rx => {
                    // Feedback cancelled, don't show popup
                }
            }
        });

        feedback_tx
    }
}
