use crossterm::event::{Event as CrostermEvent, KeyCode, KeyEvent, KeyModifiers};
use tui_input::backend::crossterm::EventHandler;

use crate::ops::logs::dump_file_path;
use crate::ops::IoReqEvent;
use crate::state::view::View;
use crate::state::{
    InputState, MultiSelectMode, MultiSelectModeReason, PopupType, RdrResult, State,
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
                            state.run_command().await?;
                            state.exit_input();
                        }
                        _ => {}
                    },
                    KeyCode::Esc => {
                        if !state.resource_list.search_filter.is_empty() {
                            state.resource_list.apply_search_filter("");
                        }
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
                            let action = {
                                let popup_type = &state.popup.as_ref().unwrap().popup_type;
                                match popup_type {
                                    PopupType::RestartResourcePopup => {
                                        state.process_restart_resource_popup()
                                    }
                                    PopupType::StartMachinesPopup => {
                                        state.process_start_machines_popup()
                                    }
                                    PopupType::SuspendMachinesPopup => {
                                        state.process_suspend_machines_popup()
                                    }
                                    PopupType::StopMachinesPopup => {
                                        state.process_stop_machines_popup()
                                    }
                                    PopupType::KillMachinePopup => {
                                        state.process_kill_machine_popup()
                                    }
                                    PopupType::DestroyResourcePopup => {
                                        state.process_destroy_resource_popup()
                                    }
                                    PopupType::CordonMachinesPopup => {
                                        state.process_cordon_machines_popup()
                                    }
                                    PopupType::UncordonMachinesPopup => {
                                        state.process_uncordon_machines_popup()
                                    }
                                    PopupType::InfoPopup
                                    | PopupType::ErrorPopup
                                    | PopupType::ViewAppReleasesPopup
                                    | PopupType::ViewAppServicesPopup => Ok(None),
                                }
                            };
                            if let Ok(action) = action {
                                state.popup = None;
                                if let Some(event) = action {
                                    if matches!(
                                        event,
                                        IoReqEvent::RestartMachines { .. }
                                            | IoReqEvent::StartMachines { .. }
                                            | IoReqEvent::SuspendMachines { .. }
                                            | IoReqEvent::StopMachines { .. }
                                            | IoReqEvent::CordonMachines { .. }
                                            | IoReqEvent::UncordonMachines { .. }
                                            | IoReqEvent::UnsetSecrets { .. }
                                    ) {
                                        state.exit_multi_select();
                                    }
                                    state.dispatch(event).await;
                                }
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
                match &state.get_current_view() {
                    resource_list @ (View::Organizations
                    | View::Apps { .. }
                    | View::Machines { .. }
                    | View::Volumes { .. }
                    | View::Secrets { .. }) => {
                        match (key_event.code, resource_list) {
                            // Machines
                            (KeyCode::Char('r'), View::Machines { .. }) => {
                                state.start_restart_machines();
                            }
                            (KeyCode::Char('s'), View::Machines { .. }) => {
                                state.start_start_machines();
                            }
                            (KeyCode::Char('u'), View::Machines { .. }) => {
                                state.start_suspend_machines();
                            }
                            (KeyCode::Char('t'), View::Machines { .. }) => {
                                state.start_stop_machines();
                            }
                            (KeyCode::Char('k'), View::Machines { .. })
                                if key_event.modifiers == KeyModifiers::CONTROL =>
                            {
                                state.open_kill_machine_popup();
                            }
                            (KeyCode::Char('c'), View::Machines { .. }) => {
                                state.start_cordon_machines();
                            }
                            (KeyCode::Char('C'), View::Machines { .. }) => {
                                state.start_uncordon_machines();
                            }
                            (KeyCode::Char('l'), View::Machines { .. }) => {
                                state.navigate_to_machine_logs().await?;
                            }
                            // Apps
                            (KeyCode::Char('o'), View::Apps { .. }) => {
                                let app: ListApp = state.get_selected_resource()?.into();
                                state
                                    .dispatch(IoReqEvent::OpenApp { app_name: app.name })
                                    .await;
                            }
                            (KeyCode::Char('r'), View::Apps { .. })
                                if key_event.modifiers == KeyModifiers::CONTROL =>
                            {
                                state.open_restart_resource_popup()?;
                            }
                            (KeyCode::Char('v'), View::Apps { .. }) => {
                                let app: ListApp = state.get_selected_resource()?.into();
                                state.clear_app_releases_list();
                                state
                                    .dispatch(IoReqEvent::ViewAppReleases { app_name: app.name })
                                    .await;
                                state.open_view_app_releases_popup()?;
                            }
                            (KeyCode::Char('s'), View::Apps { .. }) => {
                                let app: ListApp = state.get_selected_resource()?.into();
                                state.clear_app_services_list();
                                state
                                    .dispatch(IoReqEvent::ViewAppServices { app_name: app.name })
                                    .await;
                                state.open_view_app_services_popup()?;
                            }
                            (KeyCode::Char('l'), View::Apps { .. }) => {
                                state.navigate_to_app_logs().await?;
                            }
                            // Secrets
                            (KeyCode::Char('u'), View::Secrets { .. }) => {
                                state.start_unset_secrets();
                            }
                            (KeyCode::Enter, view) => {
                                if let MultiSelectMode::On(reason) = &state.multi_select_mode {
                                    if !state.resource_list.multi_select_state.is_empty() {
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
                                } else {
                                    match view {
                                        View::Machines { .. } => {
                                            state.navigate_to_machine_logs().await?;
                                        }
                                        View::Apps { .. } => {
                                            state.navigate_to_machines().await?;
                                        }
                                        View::Organizations { .. } => {
                                            state.navigate_to_apps().await?;
                                        }
                                        _ => {}
                                    }
                                }
                            }
                            (KeyCode::Char('d'), view)
                                if key_event.modifiers == KeyModifiers::CONTROL =>
                            {
                                if !matches!(view, View::Secrets { .. }) {
                                    state.open_destroy_resource_popup()?;
                                }
                            }
                            (KeyCode::Char('/'), _) => {
                                state.enter_search_mode();
                            }
                            (KeyCode::Char(' '), _) => {
                                if matches!(state.multi_select_mode, MultiSelectMode::On(..)) {
                                    state.resource_list.toggle_multi_selection();
                                }
                            }
                            (KeyCode::Esc, _) => {
                                if !state.resource_list.search_filter.is_empty() {
                                    state.resource_list.apply_search_filter("");
                                } else if matches!(state.multi_select_mode, MultiSelectMode::On(..))
                                {
                                    state.exit_multi_select();
                                } else {
                                    state.navigate_back().await?;
                                }
                            }
                            (
                                KeyCode::BackTab | KeyCode::Left | KeyCode::Up | KeyCode::Char('k'),
                                _,
                            ) => {
                                state.resource_list.previous(1);
                            }
                            (
                                KeyCode::Tab | KeyCode::Right | KeyCode::Down | KeyCode::Char('j'),
                                _,
                            ) => {
                                state.resource_list.next(1);
                            }
                            _ => {}
                        }
                    }
                    View::AppLogs { opts, .. } => match key_event.code {
                        KeyCode::Esc => state.navigate_back().await?,
                        KeyCode::PageUp => state.logs_state.transition(TuiWidgetEvent::PrevPageKey),
                        KeyCode::PageDown => {
                            state.logs_state.transition(TuiWidgetEvent::NextPageKey)
                        }
                        KeyCode::Up => state.logs_state.transition(TuiWidgetEvent::UpKey),
                        KeyCode::Down => state.logs_state.transition(TuiWidgetEvent::DownKey),
                        KeyCode::Left => state.logs_state.transition(TuiWidgetEvent::LeftKey),
                        KeyCode::Right => state.logs_state.transition(TuiWidgetEvent::RightKey),
                        KeyCode::Char('r') => {
                            state.logs_state.transition(TuiWidgetEvent::EscapeKey)
                        }
                        KeyCode::Char('+') => state.logs_state.transition(TuiWidgetEvent::PlusKey),
                        KeyCode::Char('-') => state.logs_state.transition(TuiWidgetEvent::MinusKey),
                        KeyCode::Char('t') => state.logs_state.transition(TuiWidgetEvent::HideKey),
                        KeyCode::Char('f') => state.logs_state.transition(TuiWidgetEvent::FocusKey),
                        KeyCode::Char('s') if key_event.modifiers == KeyModifiers::CONTROL => {
                            let file_path = dump_file_path(opts.app_name.clone()).await?;
                            state.dispatch(IoReqEvent::DumpLogs { file_path }).await;
                        }
                        _ => {}
                    },
                    View::MachineLogs { opts, .. } => match key_event.code {
                        KeyCode::Esc => state.navigate_back().await?,
                        KeyCode::PageUp => state.logs_state.transition(TuiWidgetEvent::PrevPageKey),
                        KeyCode::PageDown => {
                            state.logs_state.transition(TuiWidgetEvent::NextPageKey)
                        }
                        KeyCode::Char('r') => {
                            state.logs_state.transition(TuiWidgetEvent::EscapeKey)
                        }
                        KeyCode::Char('s') if key_event.modifiers == KeyModifiers::CONTROL => {
                            let file_path = dump_file_path(
                                opts.app_name.clone() + "_" + &opts.vm_id.clone().unwrap(),
                            )
                            .await?;
                            state.dispatch(IoReqEvent::DumpLogs { file_path }).await;
                        }
                        _ => {}
                    },
                }
            }
        }
    }

    Ok(())
}
