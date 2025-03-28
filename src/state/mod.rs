use std::collections::HashSet;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use color_eyre::eyre::OptionExt;
use focusable::FocusContainer;
use tokio::sync::mpsc::{self, Sender};
use tracing::{error, log};
use tui_input::Input;
use view::View;

use crate::command::{match_command, Command};
use crate::fly_rust::machine_types::{RemoveMachineInput, RestartMachineInput, StopMachineInput};
use crate::fly_rust::resource_organizations::OrganizationFilter;
use crate::fly_rust::volume_types::RemoveVolumeInput;
use crate::logs::LogOptions;
use crate::ops::apps::restart::AppRestartParams;
use crate::ops::machines::kill::KillMachineInput;
use crate::ops::{IoReqEvent, IoRespEvent};
use crate::transformations::{ListApp, ListMachine, ListOrganization, ListVolume};
use crate::widgets::focusable_check_box::CheckBox;
use crate::widgets::focusable_text::TextBox;
use crate::widgets::focusable_widget::FocusableWidget;
use crate::widgets::form::Form;
use crate::widgets::log_viewer::{LevelFilter, TuiWidgetState};
use crate::widgets::selectable_list::SelectableList;

pub mod view;

pub type RdrResult<T> = color_eyre::eyre::Result<T>;

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
    pub view_history: Vec<View>,
    pub current_view_tx: Option<Sender<View>>,
    io_tx: Option<Sender<IoReqEvent>>,
    prev_selected_id: Option<String>,
    pub resource_list: SelectableList,
    pub app_releases_list: Vec<Vec<String>>,
    pub app_services_list: Vec<Vec<String>>,
    pub logs_state: TuiWidgetState,
    pub input_state: InputState,
    pub multi_select_mode: MultiSelectMode,
    pub popup: Option<RdrPopup>,
}

impl Default for State {
    fn default() -> Self {
        Self {
            running: true,
            debugger_state: tui_logger::TuiWidgetState::new()
                .set_default_display_level(log::LevelFilter::Info),
            splash_shown: Arc::new(AtomicBool::new(false)),
            view_history: vec![View::Organizations],
            current_view_tx: None,
            io_tx: None,
            prev_selected_id: None,
            resource_list: SelectableList::default(),
            app_releases_list: vec![],
            app_services_list: vec![],
            logs_state: TuiWidgetState::new().set_default_display_level(LevelFilter::Trace),
            input_state: InputState::Hidden,
            multi_select_mode: MultiSelectMode::Off,
            popup: None,
        }
    }
}

impl State {
    pub fn init(&mut self, io_req_tx: Sender<IoReqEvent>) {
        let splash_shown = Arc::clone(&self.splash_shown);
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(500)).await;
            splash_shown.store(true, Ordering::SeqCst);
        });

        let mut current_view = self.get_current_view();
        let (current_view_tx, mut current_view_rx) = mpsc::channel::<View>(8);
        self.current_view_tx = Some(current_view_tx);
        self.io_tx = Some(io_req_tx);
        let io_tx_clone = self.io_tx.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(5));
            loop {
                tokio::select! {
                    _ = interval.tick() => {
                        match current_view {
                            View::Organizations => {
                                if let Some(io_tx) = io_tx_clone.as_ref() {
                                    let _ = io_tx.send(IoReqEvent::ListOrganizations{ filter: OrganizationFilter::default() }).await;
                                }
                            }
                            View::Apps{ref org_slug, ..} => {
                                if let Some(io_tx) = io_tx_clone.as_ref() {
                                    let _ = io_tx.send(IoReqEvent::ListApps{ org_slug: org_slug.clone() }).await;
                                }
                            }
                            View::Machines{ref app_name, ..} => {
                                if let Some(io_tx) = io_tx_clone.as_ref() {
                                    let _ = io_tx.send(IoReqEvent::ListMachines{ app_name: app_name.clone() }).await;
                                }
                            }
                            View::Volumes{ref app_name,..} => {
                                if let Some(io_tx) = io_tx_clone.as_ref() {
                                    let _ = io_tx.send(IoReqEvent::ListVolumes{ app_name: app_name.clone() }).await;
                                }
                            }
                            View::Secrets{ref app_name,..} => {
                                if let Some(io_tx) = io_tx_clone.as_ref() {
                                    let _ = io_tx.send(IoReqEvent::ListSecrets{ app_name: app_name.clone() }).await;
                                }
                            }
                            _ => {}
                        };
                    }
                    Some(new_view) = current_view_rx.recv() => {
                        current_view = new_view;
                        interval = tokio::time::interval(Duration::from_secs(5));
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

    pub async fn dispatch(&self, action: IoReqEvent) {
        if let Some(io_tx) = &self.io_tx.as_ref() {
            if let Err(e) = io_tx.send(action).await {
                error!("Error from dispatch {}", e);
            };
        }
    }

    pub async fn handle_io_resp(&mut self, io_event: IoRespEvent) {
        match io_event {
            IoRespEvent::Organizations { list }
                if matches!(self.get_current_view(), View::Organizations) =>
            {
                self.resource_list
                    .set_items(list, self.prev_selected_id.take());
            }
            IoRespEvent::Apps { list } if matches!(self.get_current_view(), View::Apps { .. }) => {
                self.resource_list
                    .set_items(list, self.prev_selected_id.take());
            }
            IoRespEvent::Machines { list }
                if matches!(self.get_current_view(), View::Machines { .. }) =>
            {
                self.resource_list
                    .set_items(list, self.prev_selected_id.take());
            }
            IoRespEvent::Volumes { list }
                if matches!(self.get_current_view(), View::Volumes { .. }) =>
            {
                self.resource_list
                    .set_items(list, self.prev_selected_id.take());
            }
            IoRespEvent::Secrets { list }
                if matches!(self.get_current_view(), View::Secrets { .. }) =>
            {
                self.resource_list
                    .set_items(list, self.prev_selected_id.take());
            }
            IoRespEvent::AppReleases { list } => {
                self.app_releases_list = list;
            }
            IoRespEvent::AppServices { list } => {
                self.app_services_list = list;
            }
            IoRespEvent::SetPopup {
                popup_type,
                message,
            } => {
                self.popup = Some(RdrPopup::new(popup_type, message));
            }
            _ => {}
        }
    }

    pub fn get_current_view(&self) -> View {
        self.view_history.last().unwrap().clone()
    }
    pub fn get_current_org(&self) -> Option<(String, String)> {
        self.view_history.iter().rev().find_map(|view| {
            if let View::Apps { org_id, org_slug } = view {
                Some((org_id.clone(), org_slug.clone()))
            } else {
                None
            }
        })
    }
    pub fn get_current_app(&self) -> Option<(String, String)> {
        self.view_history.iter().rev().find_map(|view| match view {
            View::Machines { app_id, app_name } => Some((app_id.clone(), app_name.clone())),
            View::Secrets { app_id, app_name } => Some((app_id.clone(), app_name.clone())),
            View::Volumes { app_id, app_name } => Some((app_id.clone(), app_name.clone())),
            View::AppLogs { app_id, opts } => Some((app_id.clone(), opts.app_name.clone())),
            _ => None,
        })
    }

    pub fn get_selected_resource(&self) -> RdrResult<Vec<String>> {
        self.resource_list
            .selected()
            .cloned()
            .ok_or_eyre("Selected resource is empty.")
    }

    // Navigation handling
    pub fn get_breadcrumbs(&self) -> Vec<String> {
        self.view_history
            .iter()
            .map(|view| view.to_breadcrumb())
            .collect()
    }
    pub fn get_scopes(&self) -> Vec<String> {
        self.view_history
            .iter()
            .map(|view| view.to_scope())
            .collect()
    }
    pub async fn navigate_back(&mut self) -> RdrResult<()> {
        if self.view_history.len() > 1 {
            let prev_view = self.view_history.pop();
            if let Some(prev_view) = prev_view {
                match prev_view {
                    View::Apps { org_id, .. } => {
                        self.prev_selected_id = Some(org_id);
                    }
                    View::AppLogs { app_id, .. }
                    | View::Machines { app_id, .. }
                    | View::Volumes { app_id, .. }
                    | View::Secrets { app_id, .. } => {
                        self.prev_selected_id = Some(app_id);
                    }
                    View::MachineLogs { opts, .. } => {
                        let machine_id = opts.vm_id.clone().unwrap();
                        self.prev_selected_id = Some(machine_id);
                    }
                    _ => {}
                }
            }
            self.set_current_view(&self.view_history.last().unwrap().clone())
                .await?;
        }
        Ok(())
    }
    pub async fn navigate_to_apps(&mut self) -> RdrResult<()> {
        let org: ListOrganization = self.get_selected_resource()?.into();
        let new_view = View::Apps {
            org_id: org.id,
            org_slug: org.slug,
        };
        self.set_current_view(&new_view).await?;
        self.view_history.push(new_view);
        Ok(())
    }
    pub async fn navigate_to_machines(&mut self) -> RdrResult<()> {
        let app: ListApp = self.get_selected_resource()?.into();
        let new_view = View::Machines {
            app_id: app.id,
            app_name: app.name,
        };
        self.set_current_view(&new_view).await?;
        self.view_history.push(new_view);
        Ok(())
    }
    pub async fn navigate_to_app_logs(&mut self) -> RdrResult<()> {
        let app: ListApp = self.get_selected_resource()?.into();
        let opts = LogOptions {
            app_name: app.name.clone(),
            vm_id: None,
            region_code: None,
            no_tail: false,
        };
        let new_view = View::AppLogs {
            app_id: app.id,
            opts: opts.clone(),
        };
        self.set_current_view(&new_view).await?;
        self.view_history.push(new_view);
        Ok(())
    }
    pub async fn navigate_to_machine_logs(&mut self) -> RdrResult<()> {
        let (_, app_name) = self.get_current_app().ok_or_eyre("App not found.")?;
        let machine: ListMachine = self.get_selected_resource()?.into();
        let opts = LogOptions {
            app_name: app_name.clone(),
            vm_id: Some(machine.id.clone()),
            region_code: None,
            no_tail: false,
        };
        let new_view = View::MachineLogs { opts: opts.clone() };
        self.set_current_view(&new_view).await?;
        self.view_history.push(new_view);
        Ok(())
    }
    async fn navigate_via_command(&mut self, command: Command) -> RdrResult<()> {
        let can_navigate = match command {
            Command::Organizations => Ok(View::Organizations),
            Command::Apps => self
                .get_current_org()
                .map(|(org_id, org_slug)| View::Apps { org_id, org_slug })
                .ok_or("Select an organization first."),
            Command::Machines => self
                .get_current_app()
                .map(|(app_id, app_name)| View::Machines { app_id, app_name })
                .ok_or("Select an app first."),
            Command::Volumes => self
                .get_current_app()
                .map(|(app_id, app_name)| View::Volumes { app_id, app_name })
                .ok_or("Select an app first."),
            Command::Secrets => self
                .get_current_app()
                .map(|(app_id, app_name)| View::Secrets { app_id, app_name })
                .ok_or("Select an app first."),
            Command::Quit => {
                self.quit();
                return Ok(());
            }
        };
        // Check if navigation is allowed
        match can_navigate {
            Ok(new_view) => {
                self.set_current_view(&new_view).await?;
                match new_view {
                    View::Organizations => {
                        while !matches!(self.view_history.last(), Some(View::Organizations)) {
                            self.view_history.pop();
                        }
                    }
                    View::Apps { .. } => {
                        while !matches!(self.view_history.last(), Some(View::Apps { .. })) {
                            self.view_history.pop();
                        }
                    }
                    View::Machines { .. } | View::Volumes { .. } | View::Secrets { .. } => {
                        while !matches!(self.view_history.last(), Some(View::Apps { .. })) {
                            self.view_history.pop();
                        }
                        self.view_history.push(new_view);
                    }
                    _ => {}
                }
                Ok(())
            }
            Err(err) => {
                self.open_popup(err.to_string(), PopupType::ErrorPopup, None);
                Ok(())
            }
        }
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
    pub async fn set_current_view(&mut self, new_view: &View) -> RdrResult<()> {
        match new_view {
            View::AppLogs { ref opts, .. } => {
                self.dispatch(IoReqEvent::StreamLogs { opts: opts.clone() })
                    .await;
            }
            View::MachineLogs { ref opts, .. } => {
                self.dispatch(IoReqEvent::StreamLogs { opts: opts.clone() })
                    .await;
            }
            _ => {
                self.exit_multi_select();
                self.reset_search_filter();
                self.resource_list.reset();
                // Cleanup the possible allocated logs resources while leaving logs screen
                self.logs_state =
                    TuiWidgetState::new().set_default_display_level(LevelFilter::Trace);
                self.dispatch(IoReqEvent::StopLogs).await;
                // Reset the resource list entering a new resource view
            }
        };
        if let Some(tx) = &self.current_view_tx {
            tx.send(new_view.clone()).await?;
        }
        Ok(())
    }
    pub async fn run_command(&mut self) -> RdrResult<()> {
        if let InputState::Command { input, .. } = &self.input_state {
            match input.value().parse::<Command>() {
                Ok(command) => self.navigate_via_command(command).await?,
                Err(err) => self.open_popup(err.to_string(), PopupType::ErrorPopup, None),
            }
        }

        Ok(())
    }

    pub fn enter_search_mode(&mut self) {
        self.reset_search_filter();
        self.input_state = InputState::Search {
            input: Input::default(),
        };
    }
    //INFO:Keeps the search in place
    pub fn commit_search(&mut self) {
        self.apply_search_filter();
        self.input_state = InputState::Hidden
    }
    pub fn apply_search_filter(&mut self) {
        if let InputState::Search { input } = &mut self.input_state {
            self.resource_list.apply_search_filter(input.value());
        }
    }
    fn reset_search_filter(&mut self) {
        self.resource_list.apply_search_filter("");
    }
    // Multiselect handling
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
        self.resource_list.multi_select_state = HashSet::new();
    }
    // Popup handling
    pub fn open_popup(&mut self, message: String, popup_type: PopupType, actions: Option<Form>) {
        self.popup = Some(RdrPopup::with_actions(popup_type, message, actions));
    }
    pub fn has_popup(&self) -> bool {
        self.popup.is_some()
    }
    pub fn close_popup(&mut self) {
        self.popup = None
    }
    //INFO:Can be called only if has_popup() passes
    pub fn get_popup_type(&self) -> PopupType {
        self.popup.as_ref().unwrap().popup_type.clone()
    }
    pub fn popup_focus_previous(&mut self) {
        if let Some(popup) = self.popup.as_mut() {
            popup.actions.focus_previous();
        }
    }
    pub fn popup_focus_next(&mut self) {
        if let Some(popup) = self.popup.as_mut() {
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
        let current_view = self.get_current_view();
        if let Some(popup) = self.popup.as_mut() {
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
                    if matches!(current_view, View::Machines { .. }) =>
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
    //INFO:Can be called only if has_popup() passes
    pub fn should_process_popup(&self) -> bool {
        let actions = &self.popup.as_ref().unwrap().actions;
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
    pub fn open_destroy_resource_popup(&mut self) -> RdrResult<()> {
        let mut message = String::from("Are you sure to destroy this");
        let selected_resource = self.get_selected_resource()?;
        let current_view = self.get_current_view();
        match current_view {
            View::Apps { .. } => {
                let app: ListApp = selected_resource.into();
                message = format!("{} app: {}?", message, app.name);
            }
            View::Machines { .. } => {
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
            View::Volumes { .. } => {
                let volume: ListVolume = selected_resource.into();
                message = format!(
                    "Deleting a volume is not reversible. {} volume: {}?",
                    message, volume.id
                );

                let matches = {
                    self.resource_list
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
            View::Secrets { .. } => {
                message = String::from("Are you sure to stage unset the selected secrets?");
                message.push_str("\n\nWarning! This will be staged but won't affect VMs. Run \"fly secrets deploy\" for this app to apply the changes.");
            }
            _ => {}
        }
        self.open_popup(message, PopupType::DestroyResourcePopup, None);
        Ok(())
    }
    pub fn process_destroy_resource_popup(&self) -> RdrResult<Option<IoReqEvent>> {
        if !self.should_take_action(&self.popup.as_ref().unwrap().actions) {
            return Ok(None);
        }
        let current_view = self.get_current_view();
        match current_view {
            View::Apps { org_slug, .. } => {
                let app: ListApp = self.get_selected_resource()?.into();
                Ok(Some(IoReqEvent::DestroyApp {
                    app_name: app.name,
                    org_slug,
                }))
            }
            View::Machines { app_name, .. } => {
                let machine: ListMachine = self.get_selected_resource()?.into();
                let force = self.popup.as_ref().unwrap().actions.children[0]
                    .as_any()
                    .downcast_ref::<CheckBox>()
                    .unwrap()
                    .is_checked;
                let params = RemoveMachineInput {
                    id: machine.id,
                    kill: force,
                };
                Ok(Some(IoReqEvent::DestroyMachine { app_name, params }))
            }
            View::Volumes { app_name, .. } => {
                let volume: ListVolume = self.get_selected_resource()?.into();
                let params = RemoveVolumeInput { id: volume.id };
                Ok(Some(IoReqEvent::DestroyVolume { app_name, params }))
            }
            View::Secrets { app_name, .. } => {
                let keys = self
                    .resource_list
                    .multi_select_state
                    .clone()
                    .into_iter()
                    .collect();
                Ok(Some(IoReqEvent::UnsetSecrets { app_name, keys }))
            }
            _ => Ok(None),
        }
    }
    pub fn open_restart_resource_popup(&mut self) -> RdrResult<()> {
        let mut message = String::from("Are you sure to restart");
        let current_view = self.get_current_view();
        match current_view {
            View::Apps { .. } => {
                let app: ListApp = self.get_selected_resource()?.into();
                message = format!("{} this app: {}?", message, app.name);
            }
            View::Machines { .. } => {
                message = format!("{} the selected machines?", message);
            }
            _ => {}
        }
        self.open_popup(message, PopupType::RestartResourcePopup, None);
        Ok(())
    }
    pub fn process_restart_resource_popup(&self) -> RdrResult<Option<IoReqEvent>> {
        if !self.should_take_action(&self.popup.as_ref().unwrap().actions) {
            return Ok(None);
        }
        let current_view = self.get_current_view();
        match current_view {
            View::Apps { org_slug, .. } => {
                let app: ListApp = self.get_selected_resource()?.into();
                let params = AppRestartParams {
                    force_stop: self.popup.as_ref().unwrap().actions.children[0]
                        .as_any()
                        .downcast_ref::<CheckBox>()
                        .unwrap()
                        .is_checked,
                };
                Ok(Some(IoReqEvent::RestartApp {
                    app_name: app.name,
                    params,
                    org_slug,
                }))
            }
            View::Machines { app_name, .. } => {
                let machines = self
                    .resource_list
                    .multi_select_state
                    .clone()
                    .into_iter()
                    .collect();
                let params = RestartMachineInput {
                    force_stop: self.popup.as_ref().unwrap().actions.children[0]
                        .as_any()
                        .downcast_ref::<CheckBox>()
                        .unwrap()
                        .is_checked,
                    ..Default::default()
                };
                Ok(Some(IoReqEvent::RestartMachines {
                    app_name,
                    machines,
                    params,
                }))
            }
            _ => Ok(None),
        }
    }
    pub fn open_view_app_releases_popup(&mut self) -> RdrResult<()> {
        let app: ListApp = self.get_selected_resource()?.into();
        let message = format!("Releases of {}", app.name);
        self.open_popup(message, PopupType::ViewAppReleasesPopup, None);
        Ok(())
    }
    pub fn clear_app_releases_list(&mut self) {
        self.app_releases_list = vec![];
    }
    pub fn open_view_app_services_popup(&mut self) -> RdrResult<()> {
        let app: ListApp = self.get_selected_resource()?.into();
        let message = format!("Services of {}", app.name);
        self.open_popup(message, PopupType::ViewAppServicesPopup, None);
        Ok(())
    }
    pub fn clear_app_services_list(&mut self) {
        self.app_services_list = vec![];
    }
    pub fn open_start_machines_popup(&mut self) {
        let message = String::from("Are you sure to start the selected machines?");
        self.open_popup(message, PopupType::StartMachinesPopup, None);
    }
    pub fn process_start_machines_popup(&self) -> RdrResult<Option<IoReqEvent>> {
        if !self.should_take_action(&self.popup.as_ref().unwrap().actions) {
            Ok(None)
        } else {
            let machines = self
                .resource_list
                .multi_select_state
                .clone()
                .into_iter()
                .collect();
            let (_, app_name) = self.get_current_app().ok_or_eyre("App not found.")?;
            Ok(Some(IoReqEvent::StartMachines { app_name, machines }))
        }
    }
    pub fn open_suspend_machines_popup(&mut self) {
        let message = String::from("Are you sure to suspend the selected machines?");
        self.open_popup(message, PopupType::SuspendMachinesPopup, None);
    }
    pub fn process_suspend_machines_popup(&self) -> RdrResult<Option<IoReqEvent>> {
        if !self.should_take_action(&self.popup.as_ref().unwrap().actions) {
            Ok(None)
        } else {
            let machines = self
                .resource_list
                .multi_select_state
                .clone()
                .into_iter()
                .collect();
            let (_, app_name) = self.get_current_app().ok_or_eyre("App not found.")?;
            Ok(Some(IoReqEvent::SuspendMachines { app_name, machines }))
        }
    }
    pub fn open_stop_machines_popup(&mut self) {
        let message = String::from("Are you sure to stop the selected machines?");
        self.open_popup(message, PopupType::StopMachinesPopup, None);
    }
    pub fn process_stop_machines_popup(&self) -> RdrResult<Option<IoReqEvent>> {
        if !self.should_take_action(&self.popup.as_ref().unwrap().actions) {
            Ok(None)
        } else {
            let machines = self
                .resource_list
                .multi_select_state
                .clone()
                .into_iter()
                .collect();
            let (_, app_name) = self.get_current_app().ok_or_eyre("App not found.")?;
            let params = StopMachineInput {
                ..Default::default()
            };
            Ok(Some(IoReqEvent::StopMachines {
                app_name,
                machines,
                params,
            }))
        }
    }
    pub fn open_kill_machine_popup(&mut self) {
        let message = String::from("Are you sure to kill this machine?");
        self.open_popup(message, PopupType::KillMachinePopup, None);
    }
    pub fn process_kill_machine_popup(&self) -> RdrResult<Option<IoReqEvent>> {
        if !self.should_take_action(&self.popup.as_ref().unwrap().actions) {
            Ok(None)
        } else {
            let machine: ListMachine = self.resource_list.selected().cloned().unwrap().into();
            let (_, app_name) = self.get_current_app().ok_or_eyre("App not found.")?;
            let params = KillMachineInput { id: machine.id };
            Ok(Some(IoReqEvent::KillMachine { app_name, params }))
        }
    }
    pub fn open_cordon_machines_popup(&mut self) {
        let message = String::from("Are you sure to cordon the selected machines?");
        self.open_popup(message, PopupType::CordonMachinesPopup, None);
    }
    pub fn process_cordon_machines_popup(&self) -> RdrResult<Option<IoReqEvent>> {
        if !self.should_take_action(&self.popup.as_ref().unwrap().actions) {
            Ok(None)
        } else {
            let machines = self
                .resource_list
                .multi_select_state
                .clone()
                .into_iter()
                .collect();
            let (_, app_name) = self.get_current_app().ok_or_eyre("App not found.")?;
            Ok(Some(IoReqEvent::CordonMachines { app_name, machines }))
        }
    }
    pub fn open_uncordon_machines_popup(&mut self) {
        let message = String::from("Are you sure to uncordon the selected machines?");
        self.open_popup(message, PopupType::UncordonMachinesPopup, None);
    }
    pub fn process_uncordon_machines_popup(&self) -> RdrResult<Option<IoReqEvent>> {
        if !self.should_take_action(&self.popup.as_ref().unwrap().actions) {
            Ok(None)
        } else {
            let machines = self
                .resource_list
                .multi_select_state
                .clone()
                .into_iter()
                .collect();
            let (_, app_name) = self.get_current_app().ok_or_eyre("App not found.")?;
            Ok(Some(IoReqEvent::UncordonMachines { app_name, machines }))
        }
    }
}
