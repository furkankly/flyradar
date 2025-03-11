use crossterm::event::{Event as CrostermEvent, KeyCode, KeyEvent, KeyModifiers};
use tui_input::backend::crossterm::EventHandler;

use crate::ops::logs::dump_file_path;
use crate::ops::IoEvent;
use crate::state::{
    CurrentScope, CurrentView, InputState, MultiSelectMode, MultiSelectModeReason, PopupType,
    RdrResult, State,
};
use crate::transformations::ListApp;
use crate::widgets::log_viewer::TuiWidgetEvent;

pub async fn handle_key_events(key_event: KeyEvent, state: &mut State) -> RdrResult<()> {
    match key_event.code {
        KeyCode::Char('c') | KeyCode::Char('C') if key_event.modifiers == KeyModifiers::CONTROL => {
            state.quit();
        }
        _ => {
            if !matches!(state.input_state, InputState::Hidden) {
                match key_event.code {
                    KeyCode::Enter => match &state.input_state {
                        InputState::Search { .. } => {
                            state.commit_search();
                        }
                        InputState::Command { .. } => {
                            state.run_command().await;
                            state.exit_input();
                        }
                        _ => {}
                    },
                    KeyCode::Esc => {
                        state.with_resource_list(|resource_list| {
                            if !resource_list.search_filter.is_empty() {
                                resource_list.apply_search_filter("");
                            }
                        });
                        state.exit_input();
                    }
                    KeyCode::Tab if matches!(&state.input_state, InputState::Command { .. }) => {
                        state.complete_command();
                    }
                    _ => match &mut state.input_state {
                        InputState::Search { input } => {
                            input.handle_event(&CrostermEvent::Key(key_event));
                            state.apply_search_filter();
                        }
                        InputState::Command { input, command: _ } => {
                            input.handle_event(&CrostermEvent::Key(key_event));
                            state.set_command();
                        }
                        _ => {}
                    },
                }
            } else if state.has_popup() {
                match key_event.code {
                    KeyCode::Enter => {
                        if state.should_process_popup() {
                            let action = state.process_popup(|shared_state| {
                                let popup_type = &shared_state.popup.as_ref().unwrap().popup_type;
                                match popup_type {
                                    PopupType::RestartResourcePopup => {
                                        state.process_restart_resource_popup(shared_state)
                                    }
                                    PopupType::StartMachinesPopup => {
                                        state.process_start_machines_popup(shared_state)
                                    }
                                    PopupType::SuspendMachinesPopup => {
                                        state.process_suspend_machines_popup(shared_state)
                                    }
                                    PopupType::StopMachinesPopup => {
                                        state.process_stop_machines_popup(shared_state)
                                    }
                                    PopupType::KillMachinePopup => {
                                        state.process_kill_machine_popup(shared_state)
                                    }
                                    PopupType::DestroyResourcePopup => {
                                        state.process_destroy_resource_popup(shared_state)
                                    }
                                    PopupType::CordonMachinesPopup => {
                                        state.process_cordon_machines_popup(shared_state)
                                    }
                                    PopupType::UncordonMachinesPopup => {
                                        state.process_uncordon_machines_popup(shared_state)
                                    }
                                    PopupType::InfoPopup
                                    | PopupType::ErrorPopup
                                    | PopupType::ViewAppReleasesPopup
                                    | PopupType::ViewAppServicesPopup => Ok(None),
                                }
                            });
                            //INFO: Action upon closing the popup
                            if let Some(event) = action {
                                if matches!(
                                    event,
                                    IoEvent::RestartMachines(..)
                                        | IoEvent::StartMachines(..)
                                        | IoEvent::SuspendMachines(..)
                                        | IoEvent::StopMachines(..)
                                        | IoEvent::CordonMachines(..)
                                        | IoEvent::UncordonMachines(..)
                                        | IoEvent::UnsetSecrets(..)
                                ) {
                                    state.exit_multi_select();
                                }
                                state.dispatch(event).await;
                            }
                        }
                    }
                    KeyCode::Esc => {
                        state.close_popup();
                    }
                    KeyCode::BackTab | KeyCode::Left | KeyCode::Up | KeyCode::Char('k') => {
                        state.popup_focus_previous();
                    }
                    KeyCode::Tab | KeyCode::Right | KeyCode::Down | KeyCode::Char('j') => {
                        state.popup_focus_next();
                    }
                    KeyCode::Char(' ') => {
                        state.toggle_force_checkbox();
                    }
                    _ => {}
                }
            } else {
                match key_event.code {
                    KeyCode::Char(':') => state.enter_command_mode(),
                    KeyCode::Char('n') => state
                        .debugger_state
                        .transition(tui_logger::TuiWidgetEvent::PrevPageKey),
                    KeyCode::Char('m') => state
                        .debugger_state
                        .transition(tui_logger::TuiWidgetEvent::NextPageKey),
                    KeyCode::Char('b') => state
                        .debugger_state
                        .transition(tui_logger::TuiWidgetEvent::EscapeKey),
                    _ => {}
                }
                match &state.current_view {
                    CurrentView::ResourceList(scope) => match (key_event.code, scope) {
                        // Machines
                        (KeyCode::Char('r'), CurrentScope::Machines) => {
                            state.start_restart_machines();
                        }
                        (KeyCode::Char('s'), CurrentScope::Machines) => {
                            state.start_start_machines();
                        }
                        (KeyCode::Char('u'), CurrentScope::Machines) => {
                            state.start_suspend_machines();
                        }
                        (KeyCode::Char('t'), CurrentScope::Machines) => {
                            state.start_stop_machines();
                        }
                        (KeyCode::Char('k'), CurrentScope::Machines)
                            if key_event.modifiers == KeyModifiers::CONTROL =>
                        {
                            state.open_kill_machine_popup();
                        }
                        (KeyCode::Char('c'), CurrentScope::Machines) => {
                            state.start_cordon_machines();
                        }
                        (KeyCode::Char('C'), CurrentScope::Machines) => {
                            state.start_uncordon_machines();
                        }
                        (KeyCode::Char('l'), CurrentScope::Machines) => {
                            let logs_action = state.stream_machine_logs().await?;
                            state.dispatch(logs_action).await;
                        }
                        // Apps
                        (KeyCode::Enter, CurrentScope::Apps) => {
                            state.set_current_app().await?;
                        }
                        (KeyCode::Char('o'), CurrentScope::Apps) => {
                            let app: ListApp = state.get_selected_resource()?.into();
                            state.dispatch(IoEvent::OpenApp(app.name)).await;
                        }
                        (KeyCode::Char('r'), CurrentScope::Apps)
                            if key_event.modifiers == KeyModifiers::CONTROL =>
                        {
                            state.open_restart_resource_popup()?;
                        }
                        (KeyCode::Char('v'), CurrentScope::Apps) => {
                            let app: ListApp = state.get_selected_resource()?.into();
                            state.clear_app_releases_list();
                            state.dispatch(IoEvent::ViewAppReleases(app.name)).await;
                            state.open_view_app_releases_popup()?;
                        }
                        (KeyCode::Char('s'), CurrentScope::Apps) => {
                            let app: ListApp = state.get_selected_resource()?.into();
                            state.clear_app_services_list();
                            state.dispatch(IoEvent::ViewAppServices(app.name)).await;
                            state.open_view_app_services_popup()?;
                        }
                        (KeyCode::Char('l'), CurrentScope::Apps) => {
                            let logs_action = state.stream_app_logs().await?;
                            state.dispatch(logs_action).await;
                        }
                        // Secrets
                        (KeyCode::Char('u'), CurrentScope::Secrets) => {
                            state.start_unset_secrets();
                        }
                        // Common
                        (KeyCode::Enter, _) => {
                            if let MultiSelectMode::On(reason) = &state.multi_select_mode {
                                let has_selected_items =
                                    state.with_resource_list(|resource_list| {
                                        !resource_list.multi_select_state.is_empty()
                                    });

                                if has_selected_items {
                                    match reason {
                                        MultiSelectModeReason::RestartMachines => {
                                            state.open_restart_resource_popup()?
                                        }
                                        MultiSelectModeReason::StartMachines => {
                                            state.open_start_machines_popup()
                                        }
                                        MultiSelectModeReason::SuspendMachines => {
                                            state.open_suspend_machines_popup()
                                        }
                                        MultiSelectModeReason::StopMachines => {
                                            state.open_stop_machines_popup()
                                        }
                                        MultiSelectModeReason::CordonMachines => {
                                            state.open_cordon_machines_popup()
                                        }
                                        MultiSelectModeReason::UncordonMachines => {
                                            state.open_uncordon_machines_popup()
                                        }
                                        MultiSelectModeReason::UnsetSecrets => {
                                            state.open_destroy_resource_popup()?;
                                        }
                                    }
                                }
                            }
                        }
                        (KeyCode::Char('d'), scope)
                            if key_event.modifiers == KeyModifiers::CONTROL =>
                        {
                            if !matches!(scope, CurrentScope::Secrets) {
                                state.open_destroy_resource_popup()?;
                            }
                        }
                        (KeyCode::Char('/'), _) => {
                            state.enter_search_mode();
                        }
                        (KeyCode::Char(' '), _) => {
                            if !matches!(state.multi_select_mode, MultiSelectMode::Off) {
                                state.with_resource_list(|resource_list| {
                                    resource_list.toggle_multi_selection();
                                })
                            }
                        }
                        (KeyCode::Esc, _) => {
                            state.with_resource_list(|resource_list| {
                                if !resource_list.search_filter.is_empty() {
                                    resource_list.apply_search_filter("");
                                }
                            });
                            if !matches!(state.multi_select_mode, MultiSelectMode::Off) {
                                state.exit_multi_select();
                            }
                        }
                        (
                            KeyCode::BackTab | KeyCode::Left | KeyCode::Up | KeyCode::Char('k'),
                            _,
                        ) => {
                            state.with_resource_list(|resource_list| resource_list.previous(1));
                        }
                        (KeyCode::Tab | KeyCode::Right | KeyCode::Down | KeyCode::Char('j'), _) => {
                            state.with_resource_list(|resource_list| resource_list.next(1));
                        }
                        _ => {}
                    },
                    CurrentView::AppLogs(log_opts) => match key_event.code {
                        KeyCode::Esc => state.logs_state.transition(TuiWidgetEvent::EscapeKey),
                        KeyCode::PageUp => state.logs_state.transition(TuiWidgetEvent::PrevPageKey),
                        KeyCode::PageDown => {
                            state.logs_state.transition(TuiWidgetEvent::NextPageKey)
                        }
                        KeyCode::Up => state.logs_state.transition(TuiWidgetEvent::UpKey),
                        KeyCode::Down => state.logs_state.transition(TuiWidgetEvent::DownKey),
                        KeyCode::Left => state.logs_state.transition(TuiWidgetEvent::LeftKey),
                        KeyCode::Right => state.logs_state.transition(TuiWidgetEvent::RightKey),
                        KeyCode::Char('+') => state.logs_state.transition(TuiWidgetEvent::PlusKey),
                        KeyCode::Char('-') => state.logs_state.transition(TuiWidgetEvent::MinusKey),
                        KeyCode::Char('t') => state.logs_state.transition(TuiWidgetEvent::HideKey),
                        KeyCode::Char('f') => state.logs_state.transition(TuiWidgetEvent::FocusKey),
                        KeyCode::Char('s') if key_event.modifiers == KeyModifiers::CONTROL => {
                            let file_path = dump_file_path(log_opts.app_name.clone()).await?;
                            state.dispatch(IoEvent::DumpLogs(file_path)).await;
                        }
                        _ => {}
                    },
                    CurrentView::MachineLogs(log_opts) => match key_event.code {
                        KeyCode::Esc => state.logs_state.transition(TuiWidgetEvent::EscapeKey),
                        KeyCode::PageUp => state.logs_state.transition(TuiWidgetEvent::PrevPageKey),
                        KeyCode::PageDown => {
                            state.logs_state.transition(TuiWidgetEvent::NextPageKey)
                        }
                        KeyCode::Char('s') if key_event.modifiers == KeyModifiers::CONTROL => {
                            let file_path = dump_file_path(
                                log_opts.app_name.clone() + "_" + &log_opts.vm_id.clone().unwrap(),
                            )
                            .await?;
                            state.dispatch(IoEvent::DumpLogs(file_path)).await;
                        }
                        _ => {}
                    },
                }
            }
        }
    }

    Ok(())
}
