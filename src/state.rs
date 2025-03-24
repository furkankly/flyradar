use std::collections::HashSet;
use std::fmt::{
    Display, {self},
};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use color_eyre::eyre::{eyre, OptionExt};
use focusable::FocusContainer;
use tracing::{error, log};
use tui_input::Input;

use crate::command::{match_command, Command};
use crate::fly_rust::machine_types::{RemoveMachineInput, RestartMachineInput, StopMachineInput};
use crate::fly_rust::resource_organizations::OrganizationFilter;
use crate::fly_rust::volume_types::RemoveVolumeInput;
use crate::logs::entry::LogEntry;
use crate::logs::LogOptions;
use crate::ops::apps::restart::AppRestartParams;
use crate::ops::machines::kill::KillMachineInput;
use crate::ops::IoEvent;
use crate::transformations::{ListApp, ListMachine, ListVolume};
use crate::widgets::focusable_check_box::CheckBox;
use crate::widgets::focusable_text::TextBox;
use crate::widgets::focusable_widget::FocusableWidget;
use crate::widgets::form::Form;
use crate::widgets::log_viewer::{LevelFilter, TuiWidgetState};
use crate::widgets::selectable_list::SelectableList;

pub type RdrResult<T> = color_eyre::eyre::Result<T>;

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum CurrentView {
    ResourceList(CurrentScope),
    AppLogs(LogOptions),
    MachineLogs(LogOptions),
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum CurrentScope {
    Organizations,
    Apps,
    Machines,
    Volumes,
    Secrets,
}

impl CurrentScope {
    pub fn headers(&self) -> &[&str] {
        match self {
            CurrentScope::Organizations => &["Name", "Slug", "Type"],
            CurrentScope::Apps => &["Name", "Organization", "Status", "Latest Deployment"],
            CurrentScope::Machines => &["Name", "State", "Region", "Updated At"],
            CurrentScope::Volumes => &[
                "Id",
                "State",
                "Name",
                "Size",
                "Region",
                "Zone",
                "Encrypted",
                "Attached VM",
                "Created At",
            ],
            CurrentScope::Secrets => &["Name", "Digest", "Created At"],
        }
    }
}

impl Display for CurrentScope {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CurrentScope::Organizations => write!(f, "Organizations"),
            CurrentScope::Apps => write!(f, "Apps"),
            CurrentScope::Machines => write!(f, "Machines"),
            CurrentScope::Volumes => write!(f, "Volumes"),
            CurrentScope::Secrets => write!(f, "Secrets"),
        }
    }
}

pub enum ResourceUpdate {
    Organizations(Vec<Vec<String>>),
    Apps(Vec<Vec<String>>),
    Machines(Vec<Vec<String>>),
    Volumes(Vec<Vec<String>>),
    Secrets(Vec<Vec<String>>),
}

#[derive(Debug, Clone)]
pub enum PopupType {
    ErrorPopup,
    InfoPopup,
    DestroyResourcePopup,
    RestartResourcePopup,
    ViewAppReleasesPopup,
    ViewAppServicesPopup,
    StartMachinesPopup,
    StopMachinesPopup,
    KillMachinePopup,
    SuspendMachinesPopup,
    CordonMachinesPopup,
    UncordonMachinesPopup,
}
pub struct RdrPopup {
    pub popup_type: PopupType,
    pub message: String,
    pub actions: Form,
}
impl RdrPopup {
    pub fn new(popup_type: PopupType, message: String) -> Self {
        Self::with_actions(popup_type, message, None)
    }

    pub fn with_actions(popup_type: PopupType, message: String, actions: Option<Form>) -> Self {
        let mut actions = actions.unwrap_or_else(|| match popup_type {
            PopupType::RestartResourcePopup => Form::from_iter([
                CheckBox::new("Force", false).boxed(),
                TextBox::new("Cancel").boxed(),
                TextBox::new("OK").boxed(),
            ]),
            PopupType::DestroyResourcePopup
            | PopupType::StartMachinesPopup
            | PopupType::SuspendMachinesPopup
            | PopupType::StopMachinesPopup
            | PopupType::KillMachinePopup
            | PopupType::CordonMachinesPopup
            | PopupType::UncordonMachinesPopup => {
                Form::from_iter([TextBox::new("Cancel").boxed(), TextBox::new("OK").boxed()])
            }
            PopupType::InfoPopup
            | PopupType::ErrorPopup
            | PopupType::ViewAppReleasesPopup
            | PopupType::ViewAppServicesPopup => Form::from_iter([TextBox::new("Dismiss").boxed()]),
        });

        actions.reset_focus();
        actions.focus_first();
        Self {
            popup_type,
            message,
            actions,
        }
    }
}

pub enum InputState {
    Hidden,
    Command { input: Input, command: String },
    Search { input: Input },
}

pub enum MultiSelectModeReason {
    RestartMachines,
    StartMachines,
    SuspendMachines,
    StopMachines,
    CordonMachines,
    UncordonMachines,
    UnsetSecrets,
}
pub enum MultiSelectMode {
    Off,
    On(MultiSelectModeReason),
}

pub struct State {
    pub running: bool,
    pub debugger_state: tui_logger::TuiWidgetState,
    pub splash_shown: Arc<AtomicBool>,
    pub current_view: CurrentView,
    pub input_state: InputState,
    pub current_view_tx: Option<tokio::sync::mpsc::Sender<CurrentView>>,
    io_tx: Option<tokio::sync::mpsc::Sender<IoEvent>>,
    pub multi_select_mode: MultiSelectMode,
    pub logs_state: TuiWidgetState,
    pub shared_state: Arc<Mutex<SharedState>>,
}

pub struct SharedState {
    pub current_app: Option<String>,
    pub resource_list_tx: Option<tokio::sync::mpsc::Sender<ResourceUpdate>>,
    pub resource_list: SelectableList,
    pub app_releases_list: Vec<Vec<String>>,
    pub app_services_list: Vec<Vec<String>>,
    // INFO: logs_state is used together with Drain api of logger viewer instead of this.
    pub logs_rx: Option<tokio::sync::mpsc::Receiver<RdrResult<LogEntry>>>,
    pub popup: Option<RdrPopup>,
}

impl Default for State {
    fn default() -> Self {
        Self {
            running: true,
            debugger_state: tui_logger::TuiWidgetState::new()
                .set_default_display_level(log::LevelFilter::Info),
            splash_shown: Arc::new(AtomicBool::new(false)),
            current_view: CurrentView::ResourceList(CurrentScope::Organizations),
            input_state: InputState::Hidden,
            current_view_tx: None,
            io_tx: None,
            multi_select_mode: MultiSelectMode::Off,
            logs_state: TuiWidgetState::new().set_default_display_level(LevelFilter::Trace),
            shared_state: Arc::new(Mutex::new(SharedState {
                current_app: None,
                resource_list_tx: None,
                resource_list: SelectableList::default(),
                app_releases_list: vec![],
                app_services_list: vec![],
                logs_rx: None,
                popup: None,
            })),
        }
    }
}

impl State {
    pub fn init(&mut self, io_tx: tokio::sync::mpsc::Sender<IoEvent>) {
        let splash_shown = Arc::clone(&self.splash_shown);
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(500)).await;
            splash_shown.store(true, Ordering::SeqCst);
        });

        let mut current_view = self.current_view.clone();
        let (current_view_tx, mut current_view_rx) = tokio::sync::mpsc::channel::<CurrentView>(100);
        let (resource_list_tx, mut resource_list_rx) =
            tokio::sync::mpsc::channel::<ResourceUpdate>(1000);

        {
            let mut shared_state = self.shared_state.lock().unwrap();
            shared_state.resource_list_tx = Some(resource_list_tx);
        }
        self.current_view_tx = Some(current_view_tx);
        self.io_tx = Some(io_tx);

        let io_tx_clone = self.io_tx.clone();
        let shared_state_clone = Arc::clone(&self.shared_state);
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(5));
            loop {
                tokio::select! {
                    _ = interval.tick() => {
                        if let CurrentView::ResourceList(scope) = &current_view {
                        match scope {
                            CurrentScope::Organizations => {
                                if let Some(io_tx) = io_tx_clone.as_ref() {
                                    let _ = io_tx.send(IoEvent::ListOrganizations(OrganizationFilter::default())).await;
                                }
                            }
                            CurrentScope::Apps => {
                                if let Some(io_tx) = io_tx_clone.as_ref() {
                                    let _ = io_tx.send(IoEvent::ListApps).await;
                                }
                            }
                            CurrentScope::Machines => {
                                let app_name = {
                                    let shared_state_guard = shared_state_clone.lock().unwrap();
                                    shared_state_guard.current_app.clone()
                                        .expect("Current app is empty")
                                };
                                if let Some(io_tx) = io_tx_clone.as_ref() {
                                    let _ = io_tx.send(IoEvent::ListMachines(app_name)).await;
                                }
                            }
                            CurrentScope::Volumes => {
                                let app_name = {
                                    let shared_state_guard = shared_state_clone.lock().unwrap();
                                    shared_state_guard.current_app.clone()
                                        .expect("Current app is empty")
                                };
                                if let Some(io_tx) = io_tx_clone.as_ref() {
                                    let _ = io_tx.send(IoEvent::ListVolumes(app_name)).await;
                                }
                            }
                            CurrentScope::Secrets => {
                                let app_name = {
                                    let shared_state_guard = shared_state_clone.lock().unwrap();
                                    shared_state_guard.current_app.clone()
                                        .expect("Current app is empty")
                                };
                                if let Some(io_tx) = io_tx_clone.as_ref() {
                                    let _ = io_tx.send(IoEvent::ListSecrets(app_name)).await;
                                }
                            }
                        };
                    }}
                    Some(new_view) = current_view_rx.recv() => {
                        current_view = new_view;
                        interval = tokio::time::interval(Duration::from_secs(5));
                    }
                    Some(resource_update) = resource_list_rx.recv() => {
                        // Only update if the scope matches current view
                        if let CurrentView::ResourceList(current_scope) = &current_view {
                            let should_update = matches!(
                                (&resource_update, current_scope),
                                (ResourceUpdate::Organizations(_), CurrentScope::Organizations) |
                                (ResourceUpdate::Apps(_), CurrentScope::Apps) |
                                (ResourceUpdate::Machines(_), CurrentScope::Machines) |
                                (ResourceUpdate::Volumes(_), CurrentScope::Volumes) |
                                (ResourceUpdate::Secrets(_), CurrentScope::Secrets)
                            );
                            if should_update {
                                let mut shared_state = shared_state_clone.lock().unwrap();
                                match resource_update {
                                    ResourceUpdate::Organizations(organizations) => shared_state.resource_list.set_items(organizations),
                                    ResourceUpdate::Apps(apps) => shared_state.resource_list.set_items(apps),
                                    ResourceUpdate::Machines(machines) => shared_state.resource_list.set_items(machines),
                                    ResourceUpdate::Volumes(volumes) => shared_state.resource_list.set_items(volumes),
                                    ResourceUpdate::Secrets(secrets) => shared_state.resource_list.set_items(secrets),
                                }
                            }
                        }
                    }
                }
            }
        });
    }

    /// Handles the tick event of the terminal.
    pub async fn tick(&mut self) {}

    pub fn quit(&mut self) {
        self.running = false;
    }

    pub async fn dispatch(&self, action: IoEvent) {
        if let Some(io_tx) = &self.io_tx.as_ref() {
            if let Err(e) = io_tx.send(action).await {
                error!("Error from dispatch {}", e);
            };
        }
    }

    pub fn get_selected_resource(&self) -> RdrResult<Vec<String>> {
        let shared_state_guard = self.shared_state.lock().unwrap();
        shared_state_guard
            .resource_list
            .selected()
            .cloned()
            .ok_or_eyre("Selected resource is empty.")
    }

    pub fn get_current_app_name(&self) -> Option<String> {
        let shared_state_guard = self.shared_state.lock().unwrap();
        shared_state_guard.current_app.clone()
    }

    pub async fn set_current_app(&mut self) -> RdrResult<()> {
        let has_selected_app = {
            let mut shared_state_guard = self.shared_state.lock().unwrap();
            if let Some(selected_resource) = shared_state_guard.resource_list.selected() {
                let selected_app: ListApp = selected_resource.clone().into();
                shared_state_guard.current_app = Some(selected_app.name.clone());
                true
            } else {
                false
            }
        };
        if has_selected_app {
            self.set_current_view(CurrentView::ResourceList(CurrentScope::Machines))
                .await?;
        }
        Ok(())
    }

    pub fn exit_input(&mut self) {
        self.input_state = InputState::Hidden
    }
    // Command handling
    pub fn enter_command_mode(&mut self) {
        self.reset_search_filter();
        self.input_state = InputState::Command {
            input: Input::default(),
            command: String::default(),
        };
    }
    pub fn set_command(&mut self) {
        if let InputState::Command { input, command } = &mut self.input_state {
            *command = String::from(match_command(input.value()));
        }
    }
    pub fn complete_command(&mut self) {
        if let InputState::Command { input, command } = &mut self.input_state {
            *command = String::from(match_command(input.value()));
            *input = Input::new(command.clone());
        }
    }
    pub async fn set_current_view(&mut self, new_view: CurrentView) -> RdrResult<()> {
        self.reset_search_filter();
        self.exit_multi_select();
        if matches!(new_view, CurrentView::ResourceList(_)) {
            // Cleanup the possible allocated logs resources while leaving logs screen
            self.logs_state = TuiWidgetState::new().set_default_display_level(LevelFilter::Trace);
            self.dispatch(IoEvent::StopLogs).await;
            if let CurrentView::ResourceList(scope) = new_view {
                if !(scope == CurrentScope::Apps || scope == CurrentScope::Organizations)
                    && self.get_current_app_name().is_none()
                {
                    return Err(eyre!("Please choose a fly app first."));
                }
            }
            // Reset the resource list entering a new resource view
            {
                let mut shared_state_guard = self.shared_state.lock().unwrap();
                shared_state_guard.resource_list.reset();
            }
        }
        if let Some(tx) = &self.current_view_tx {
            tx.send(new_view.clone()).await.map(|_| {
                self.current_view = new_view;
            })?;
        }
        Ok(())
    }
    pub async fn run_command(&mut self) {
        if let InputState::Command { input, command: _ } = &self.input_state {
            let command = input.value().parse::<Command>();
            let result = match command {
                Ok(Command::Organizations) => {
                    self.set_current_view(CurrentView::ResourceList(CurrentScope::Organizations))
                        .await
                }
                Ok(Command::Apps) => {
                    self.set_current_view(CurrentView::ResourceList(CurrentScope::Apps))
                        .await
                }
                Ok(Command::Machines) => {
                    self.set_current_view(CurrentView::ResourceList(CurrentScope::Machines))
                        .await
                }
                Ok(Command::Volumes) => {
                    self.set_current_view(CurrentView::ResourceList(CurrentScope::Volumes))
                        .await
                }
                Ok(Command::Secrets) => {
                    self.set_current_view(CurrentView::ResourceList(CurrentScope::Secrets))
                        .await
                }
                Ok(Command::Quit) => {
                    self.quit();
                    Ok(())
                }
                Err(err) => {
                    self.open_popup(err.to_string(), PopupType::ErrorPopup, None);
                    Ok(())
                }
            };
            if let Err(err) = result {
                self.open_popup(err.to_string(), PopupType::ErrorPopup, None);
            }
        }
    }

    pub fn enter_search_mode(&mut self) {
        self.reset_search_filter();
        self.input_state = InputState::Search {
            input: Input::default(),
        };
    }
    /// Keeps the search in place
    pub fn commit_search(&mut self) {
        self.apply_search_filter();
        self.input_state = InputState::Hidden
    }
    pub fn apply_search_filter(&mut self) {
        if let InputState::Search { input } = &mut self.input_state {
            let mut shared_state_guard = self.shared_state.lock().unwrap();
            shared_state_guard
                .resource_list
                .apply_search_filter(input.value());
        }
    }
    fn reset_search_filter(&mut self) {
        let mut shared_state_guard = self.shared_state.lock().unwrap();
        shared_state_guard.resource_list.apply_search_filter("");
    }

    pub fn start_restart_machines(&mut self) {
        self.multi_select_mode = MultiSelectMode::On(MultiSelectModeReason::RestartMachines);
    }
    pub fn start_start_machines(&mut self) {
        self.multi_select_mode = MultiSelectMode::On(MultiSelectModeReason::StartMachines);
    }
    pub fn start_suspend_machines(&mut self) {
        self.multi_select_mode = MultiSelectMode::On(MultiSelectModeReason::SuspendMachines);
    }
    pub fn start_stop_machines(&mut self) {
        self.multi_select_mode = MultiSelectMode::On(MultiSelectModeReason::StopMachines);
    }
    pub fn start_cordon_machines(&mut self) {
        self.multi_select_mode = MultiSelectMode::On(MultiSelectModeReason::CordonMachines);
    }
    pub fn start_uncordon_machines(&mut self) {
        self.multi_select_mode = MultiSelectMode::On(MultiSelectModeReason::UncordonMachines);
    }
    pub fn start_unset_secrets(&mut self) {
        self.multi_select_mode = MultiSelectMode::On(MultiSelectModeReason::UnsetSecrets);
    }
    pub fn exit_multi_select(&mut self) {
        self.multi_select_mode = MultiSelectMode::Off;
        let mut shared_state_guard = self.shared_state.lock().unwrap();
        shared_state_guard.resource_list.multi_select_state = HashSet::new();
    }

    pub fn open_popup(&mut self, message: String, popup_type: PopupType, actions: Option<Form>) {
        let mut shared_state_guard = self.shared_state.lock().unwrap();
        shared_state_guard.popup = Some(RdrPopup::with_actions(popup_type, message, actions));
    }
    pub fn has_popup(&self) -> bool {
        let shared_state_guard = self.shared_state.lock().unwrap();
        shared_state_guard.popup.is_some()
    }
    pub fn close_popup(&mut self) {
        let mut shared_state_guard = self.shared_state.lock().unwrap();
        shared_state_guard.popup = None
    }
    //INFO:Can be called only if has_popup() passes
    pub fn get_popup_type(&self) -> PopupType {
        let shared_state_guard = self.shared_state.lock().unwrap();
        shared_state_guard
            .popup
            .as_ref()
            .unwrap()
            .popup_type
            .clone()
    }
    pub fn popup_focus_previous(&mut self) {
        let mut shared_state_guard = self.shared_state.lock().unwrap();
        if let Some(popup) = shared_state_guard.popup.as_mut() {
            popup.actions.focus_previous();
        }
    }
    pub fn popup_focus_next(&mut self) {
        let mut shared_state_guard = self.shared_state.lock().unwrap();
        if let Some(popup) = shared_state_guard.popup.as_mut() {
            popup.actions.focus_next();
        }
    }
    pub fn should_take_action(&self, actions: &Form) -> bool {
        actions
            .children
            .iter()
            .find(|child| child.is_focused())
            .and_then(|focused_action| focused_action.as_any().downcast_ref::<TextBox>())
            .is_some_and(|textbox| textbox.content == "OK")
    }
    pub fn toggle_force_checkbox(&mut self) {
        let mut shared_state_guard = self.shared_state.lock().unwrap();
        if let Some(popup) = shared_state_guard.popup.as_mut() {
            match popup.popup_type {
                PopupType::RestartResourcePopup => {
                    let checkbox = popup.actions.children[0].as_mut();
                    if checkbox.is_focused() {
                        checkbox
                            .as_any_mut()
                            .downcast_mut::<CheckBox>()
                            .unwrap()
                            .toggle();
                    }
                }
                PopupType::DestroyResourcePopup
                    if self.current_view == CurrentView::ResourceList(CurrentScope::Machines) =>
                {
                    let checkbox = popup.actions.children[0].as_mut();
                    if checkbox.is_focused() {
                        checkbox
                            .as_any_mut()
                            .downcast_mut::<CheckBox>()
                            .unwrap()
                            .toggle();
                    }
                }
                _ => {}
            }
        }
    }

    ///INFO: Can be called only if has_popup() passes
    pub fn should_process_popup(&self) -> bool {
        let shared_state_guard = self.shared_state.lock().unwrap();
        let actions = &shared_state_guard.popup.as_ref().unwrap().actions;
        actions
            .children
            .iter()
            .find(|child| child.is_focused())
            .and_then(|focused_action| focused_action.as_any().downcast_ref::<TextBox>())
            .is_some_and(|textbox| {
                textbox.content == "OK"
                    || textbox.content == "Cancel"
                    || textbox.content == "Dismiss"
            })
    }
    /// INFO: Returns the IO action that needs to be taken and closes the popup
    /// Can be called only if has_popup() passes
    pub fn process_popup<F, T>(&self, f: F) -> Option<T>
    where
        F: FnOnce(&mut SharedState) -> RdrResult<Option<T>>,
    {
        let mut shared_state_guard = self.shared_state.lock().unwrap();
        match f(&mut shared_state_guard) {
            Ok(action) => {
                shared_state_guard.popup = None;
                action
            }
            Err(_) => None,
        }
    }
    pub fn open_destroy_resource_popup(&mut self) -> RdrResult<()> {
        let mut message = String::from("Are you sure to destroy this");
        let selected_resource = self.get_selected_resource()?;
        match self.current_view {
            CurrentView::ResourceList(CurrentScope::Apps) => {
                let app: ListApp = selected_resource.into();
                message = format!("{} app: {}?", message, app.name);
            }
            CurrentView::ResourceList(CurrentScope::Machines) => {
                let machine: ListMachine = selected_resource.into();
                message = format!("{} machine: {}?", message, machine.name);
                self.open_popup(
                    message,
                    PopupType::DestroyResourcePopup,
                    Some(Form::from_iter([
                        CheckBox::new("Force", false).boxed(),
                        TextBox::new("Cancel").boxed(),
                        TextBox::new("OK").boxed(),
                    ])),
                );
                return Ok(());
            }
            CurrentView::ResourceList(CurrentScope::Volumes) => {
                let volume: ListVolume = selected_resource.into();
                message = format!(
                    "Deleting a volume is not reversible. {} volume: {}?",
                    message, volume.id
                );

                let matches = {
                    let shared_state_guard = self.shared_state.lock().unwrap();
                    shared_state_guard
                        .resource_list
                        .items
                        .iter()
                        .filter(|&item| {
                            let v: ListVolume = item.clone().into();
                            v.name == volume.name
                        })
                        .count()
                };
                if matches <= 2 {
                    message.push_str(&format!("\n\nWarning! Every volume is pinned to a specific physical host. You should create two or more volumes per application. Deleting this volume will leave you with {} volume(s) for this application, and it is not reversible.\n\nLearn more at https://fly.io/docs/volumes/overview/",matches -1));
                }
            }
            CurrentView::ResourceList(CurrentScope::Secrets) => {
                message = String::from("Are you sure to stage unset the selected secrets?");
                message.push_str("\n\nWarning! This will be staged but won't affect VMs. Run \"fly secrets deploy\" for this app to apply the changes.");
            }
            _ => {}
        }
        self.open_popup(message, PopupType::DestroyResourcePopup, None);
        Ok(())
    }
    pub fn process_destroy_resource_popup(
        &self,
        shared_state: &mut SharedState,
    ) -> RdrResult<Option<IoEvent>> {
        if !self.should_take_action(&shared_state.popup.as_ref().unwrap().actions) {
            return Ok(None);
        }
        match self.current_view {
            CurrentView::ResourceList(CurrentScope::Apps) => {
                let app: ListApp = shared_state
                    .resource_list
                    .selected()
                    .cloned()
                    .unwrap()
                    .into();
                Ok(Some(IoEvent::DestroyApp(app.name)))
            }
            CurrentView::ResourceList(CurrentScope::Machines) => {
                let app_name = shared_state
                    .current_app
                    .clone()
                    .ok_or_eyre("Current app is empty while trying to destroy machine.")?;
                let machine: ListMachine = shared_state
                    .resource_list
                    .selected()
                    .cloned()
                    .unwrap()
                    .into();
                let force = shared_state.popup.as_ref().unwrap().actions.children[0]
                    .as_any()
                    .downcast_ref::<CheckBox>()
                    .unwrap()
                    .is_checked;
                let params = RemoveMachineInput {
                    id: machine.id,
                    kill: force,
                };
                Ok(Some(IoEvent::DestroyMachine(app_name, params)))
            }
            CurrentView::ResourceList(CurrentScope::Volumes) => {
                let app_name = shared_state
                    .current_app
                    .clone()
                    .ok_or_eyre("Current app is empty while trying to destroy machine.")?;
                let volume: ListVolume = shared_state
                    .resource_list
                    .selected()
                    .cloned()
                    .unwrap()
                    .into();
                let params = RemoveVolumeInput { id: volume.id };
                Ok(Some(IoEvent::DestroyVolume(app_name, params)))
            }
            CurrentView::ResourceList(CurrentScope::Secrets) => {
                let secrets = shared_state
                    .resource_list
                    .multi_select_state
                    .clone()
                    .into_iter()
                    .collect();
                let app_name = shared_state
                    .current_app
                    .clone()
                    .ok_or_eyre("Current app is empty while trying to stage unset secrets.")?;
                Ok(Some(IoEvent::UnsetSecrets(app_name, secrets)))
            }
            _ => Ok(None),
        }
    }
    pub fn open_restart_resource_popup(&mut self) -> RdrResult<()> {
        let mut message = String::from("Are you sure to restart");
        let selected_resource = self.get_selected_resource()?;
        match self.current_view {
            CurrentView::ResourceList(CurrentScope::Apps) => {
                let app: ListApp = selected_resource.into();
                message = format!("{} this app: {}?", message, app.name);
            }
            CurrentView::ResourceList(CurrentScope::Machines) => {
                message = format!("{} the selected machines?", message);
            }
            _ => {}
        }
        self.open_popup(message, PopupType::RestartResourcePopup, None);
        Ok(())
    }
    pub fn process_restart_resource_popup(
        &self,
        shared_state: &mut SharedState,
    ) -> RdrResult<Option<IoEvent>> {
        if !self.should_take_action(&shared_state.popup.as_ref().unwrap().actions) {
            return Ok(None);
        }
        match self.current_view {
            CurrentView::ResourceList(CurrentScope::Apps) => {
                let app: ListApp = shared_state
                    .resource_list
                    .selected()
                    .cloned()
                    .unwrap()
                    .into();
                let params = AppRestartParams {
                    force_stop: shared_state.popup.as_ref().unwrap().actions.children[0]
                        .as_any()
                        .downcast_ref::<CheckBox>()
                        .unwrap()
                        .is_checked,
                };
                Ok(Some(IoEvent::RestartApp(app.name, params)))
            }
            CurrentView::ResourceList(CurrentScope::Machines) => {
                let app_name = shared_state
                    .current_app
                    .clone()
                    .ok_or_eyre("Current app is empty while trying to restart machines.")?;
                let machines = shared_state
                    .resource_list
                    .multi_select_state
                    .clone()
                    .into_iter()
                    .collect();
                let params = RestartMachineInput {
                    force_stop: shared_state.popup.as_ref().unwrap().actions.children[0]
                        .as_any()
                        .downcast_ref::<CheckBox>()
                        .unwrap()
                        .is_checked,
                    ..Default::default()
                };
                Ok(Some(IoEvent::RestartMachines(app_name, machines, params)))
            }
            _ => Ok(None),
        }
    }
    pub fn open_view_app_releases_popup(&mut self) -> RdrResult<()> {
        let selected_resource = self.get_selected_resource()?;
        let app: ListApp = selected_resource.into();
        let message = format!("Releases of {}", app.name);
        self.open_popup(message, PopupType::ViewAppReleasesPopup, None);
        Ok(())
    }
    pub fn clear_app_releases_list(&mut self) {
        let mut shared_state_guard = self.shared_state.lock().unwrap();
        shared_state_guard.app_releases_list = vec![];
    }
    pub fn open_view_app_services_popup(&mut self) -> RdrResult<()> {
        let selected_resource = self.get_selected_resource()?;
        let app: ListApp = selected_resource.into();
        let message = format!("Services of {}", app.name);
        self.open_popup(message, PopupType::ViewAppServicesPopup, None);
        Ok(())
    }
    pub fn clear_app_services_list(&mut self) {
        let mut shared_state_guard = self.shared_state.lock().unwrap();
        shared_state_guard.app_services_list = vec![];
    }
    pub fn open_start_machines_popup(&mut self) {
        let message = String::from("Are you sure to start the selected machines?");
        self.open_popup(message, PopupType::StartMachinesPopup, None);
    }
    pub fn process_start_machines_popup(
        &self,
        shared_state: &mut SharedState,
    ) -> RdrResult<Option<IoEvent>> {
        if !self.should_take_action(&shared_state.popup.as_ref().unwrap().actions) {
            Ok(None)
        } else {
            let machines = shared_state
                .resource_list
                .multi_select_state
                .clone()
                .into_iter()
                .collect();
            let app_name = shared_state
                .current_app
                .clone()
                .ok_or_eyre("Current app is empty while trying to start machines.")?;
            Ok(Some(IoEvent::StartMachines(app_name, machines)))
        }
    }
    pub fn open_suspend_machines_popup(&mut self) {
        let message = String::from("Are you sure to suspend the selected machines?");
        self.open_popup(message, PopupType::SuspendMachinesPopup, None);
    }
    pub fn process_suspend_machines_popup(
        &self,
        shared_state: &mut SharedState,
    ) -> RdrResult<Option<IoEvent>> {
        if !self.should_take_action(&shared_state.popup.as_ref().unwrap().actions) {
            Ok(None)
        } else {
            let machines = shared_state
                .resource_list
                .multi_select_state
                .clone()
                .into_iter()
                .collect();
            let app_name = shared_state
                .current_app
                .clone()
                .ok_or_eyre("Current app is empty while trying to stop machines.")?;
            Ok(Some(IoEvent::SuspendMachines(app_name, machines)))
        }
    }
    pub fn open_stop_machines_popup(&mut self) {
        let message = String::from("Are you sure to stop the selected machines?");
        self.open_popup(message, PopupType::StopMachinesPopup, None);
    }
    pub fn process_stop_machines_popup(
        &self,
        shared_state: &mut SharedState,
    ) -> RdrResult<Option<IoEvent>> {
        if !self.should_take_action(&shared_state.popup.as_ref().unwrap().actions) {
            Ok(None)
        } else {
            let machines = shared_state
                .resource_list
                .multi_select_state
                .clone()
                .into_iter()
                .collect();
            let app_name = shared_state
                .current_app
                .clone()
                .ok_or_eyre("Current app is empty while trying to stop machines.")?;
            let params = StopMachineInput {
                ..Default::default()
            };
            Ok(Some(IoEvent::StopMachines(app_name, machines, params)))
        }
    }
    pub fn open_kill_machine_popup(&mut self) {
        let message = String::from("Are you sure to kill this machine?");
        self.open_popup(message, PopupType::KillMachinePopup, None);
    }
    pub fn process_kill_machine_popup(
        &self,
        shared_state: &mut SharedState,
    ) -> RdrResult<Option<IoEvent>> {
        if !self.should_take_action(&shared_state.popup.as_ref().unwrap().actions) {
            Ok(None)
        } else {
            let app_name = shared_state
                .current_app
                .clone()
                .ok_or_eyre("Current app is empty while trying to kill a machine.")?;
            let machine: ListMachine = shared_state
                .resource_list
                .selected()
                .cloned()
                .unwrap()
                .into();
            let params = KillMachineInput { id: machine.id };
            Ok(Some(IoEvent::KillMachine(app_name, params)))
        }
    }
    pub fn open_cordon_machines_popup(&mut self) {
        let message = String::from("Are you sure to cordon the selected machines?");
        self.open_popup(message, PopupType::CordonMachinesPopup, None);
    }
    pub fn process_cordon_machines_popup(
        &self,
        shared_state: &mut SharedState,
    ) -> RdrResult<Option<IoEvent>> {
        if !self.should_take_action(&shared_state.popup.as_ref().unwrap().actions) {
            Ok(None)
        } else {
            let machines = shared_state
                .resource_list
                .multi_select_state
                .clone()
                .into_iter()
                .collect();
            let app_name = shared_state
                .current_app
                .clone()
                .ok_or_eyre("Current app is empty while trying to cordon machines.")?;
            Ok(Some(IoEvent::CordonMachines(app_name, machines)))
        }
    }
    pub fn open_uncordon_machines_popup(&mut self) {
        let message = String::from("Are you sure to uncordon the selected machines?");
        self.open_popup(message, PopupType::UncordonMachinesPopup, None);
    }
    pub fn process_uncordon_machines_popup(
        &self,
        shared_state: &mut SharedState,
    ) -> RdrResult<Option<IoEvent>> {
        if self.should_take_action(&shared_state.popup.as_ref().unwrap().actions) {
            Ok(None)
        } else {
            let machines = shared_state
                .resource_list
                .multi_select_state
                .clone()
                .into_iter()
                .collect();
            let app_name = shared_state
                .current_app
                .clone()
                .ok_or_eyre("Current app is empty while trying to uncordon machines.")?;
            Ok(Some(IoEvent::UncordonMachines(app_name, machines)))
        }
    }
    // Logs handling
    pub async fn stream_machine_logs(&mut self) -> RdrResult<IoEvent> {
        let (app_name, machine) = {
            let shared_state_guard = self.shared_state.lock().unwrap();
            let app_name = shared_state_guard
                .current_app
                .clone()
                .ok_or_eyre("Current app is empty while trying to stream logs for the machine.")?;
            let machine: ListMachine = shared_state_guard
                .resource_list
                .selected()
                .ok_or_eyre("No selected machine to fetch logs for.")?
                .clone()
                .into();
            (app_name, machine)
        };
        let opts = LogOptions {
            app_name,
            vm_id: Some(machine.id.clone()),
            region_code: None,
            no_tail: false,
        };
        self.set_current_view(CurrentView::MachineLogs(opts.clone()))
            .await?;
        Ok(IoEvent::StreamLogs(opts))
    }
    pub async fn stream_app_logs(&mut self) -> RdrResult<IoEvent> {
        let app: ListApp = self.get_selected_resource()?.into();
        let opts = LogOptions {
            app_name: app.name.clone(),
            vm_id: None,
            region_code: None,
            no_tail: false,
        };
        self.set_current_view(CurrentView::AppLogs(opts.clone()))
            .await?;
        Ok(IoEvent::StreamLogs(opts))
    }
    pub fn with_resource_list<F, R>(&self, f: F) -> R
    where
        F: FnOnce(&mut SelectableList) -> R,
    {
        let mut shared_state_guard = self.shared_state.lock().unwrap();
        f(&mut shared_state_guard.resource_list)
    }
}
