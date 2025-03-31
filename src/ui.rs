use std::sync::atomic::Ordering;

use itertools::Itertools;
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Margin, Rect};
use ratatui::style::{Color, Style, Stylize};
use ratatui::symbols::border;
use ratatui::text::{Line, Span, Text, ToSpan, ToText};
use ratatui::widgets::{Block, Borders, Cell, Padding, Paragraph, Row, Table, TableState, Wrap};
use ratatui::Frame;
use tui_big_text::{BigText, PixelSize};
use tui_input::Input;
use unicode_width::UnicodeWidthStr;

use crate::command::{Command, COMMANDS};
use crate::state::view::View;
use crate::state::{InputState, MultiSelectMode, MultiSelectModeReason, PopupType, State};
use crate::widgets::focusable_check_box::CheckBox;
use crate::widgets::focusable_text::TextBox;
use crate::widgets::log_viewer::{TuiLoggerLevelOutput, TuiLoggerSmartWidget, TuiLoggerWidget};
use crate::widgets::popup::render_popup;
use crate::widgets::{fly_balloon, fly_visual};

pub struct Palette;

impl Palette {
    pub const DARK_PURPLE: Color = Color::Indexed(55); // #5B21B6
    pub const PURPLE: Color = Color::Indexed(93);
    pub const LIGHT_PURPLE: Color = Color::Indexed(183); // #CA7FF8
    pub const DARK_BLUE: Color = Color::Indexed(25);
    pub const BLUE: Color = Color::Indexed(33); // #1A91FF
    pub const LIGHT_BLUE: Color = Color::Indexed(75);
    pub const DARK_TEAL: Color = Color::Indexed(66);
    pub const TEAL: Color = Color::Indexed(109); // #91B9B7
    pub const LIGHT_TEAL: Color = Color::Indexed(115);
    pub const DARK_PINK: Color = Color::Indexed(198);
    pub const PINK: Color = Color::Indexed(205); //
    pub const LIGHT_PINK: Color = Color::Indexed(217); // #F9C0BE
    pub const GRAY: Color = Color::Indexed(244);
    pub const DARK_GRAY: Color = Color::Indexed(236);
}

fn render_splash(frame: &mut Frame) {
    let splash_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(8), Constraint::Min(0)])
        .split(frame.area());

    let visual_width = 104;
    let text_bg = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length(
                (splash_layout[0].width.checked_sub(visual_width)).unwrap_or_default() / 2,
            ),
            Constraint::Min(visual_width),
            Constraint::Length(
                (splash_layout[0].width.checked_sub(visual_width)).unwrap_or_default() / 2,
            ),
        ])
        .split(splash_layout[0])[1];
    let big_text = BigText::builder()
        .centered()
        .pixel_size(PixelSize::Full)
        .style(Style::new().fg(Palette::DARK_PURPLE).italic())
        .lines(vec!["flyradar".into()])
        .build();

    frame.render_widget(big_text, splash_layout[0]);
    frame.render_widget(Block::default().bg(Color::Black), text_bg);

    let visual_area = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length(
                (splash_layout[1].width.checked_sub(visual_width)).unwrap_or_default() / 2,
            ),
            Constraint::Min(visual_width),
            Constraint::Length(
                (splash_layout[1].width.checked_sub(visual_width)).unwrap_or_default() / 2,
            ),
        ])
        .split(splash_layout[1])[1];
    let fly_visual = fly_visual::FlyVisualWidget::default();
    frame.render_widget(fly_visual, visual_area);
}

fn render_header(state: &mut State, frame: &mut Frame, area: Rect) {
    let header_layout = Layout::default()
        .direction(Direction::Horizontal)
        .constraints(vec![Constraint::Min(0), Constraint::Length(24)])
        .split(area);

    let mut keymap = vec![
        ("<Ctrl-a>", "View commands"),
        (":cmd", "Command mode"),
        ("<Esc>", "Back/Cancel"),
    ];

    let current_view = state.get_current_view();
    match current_view {
        View::Organizations { ref filter } => {
            keymap = [
                &[
                    ("<Enter>", "List apps"),
                    ("<s>", "Show"),
                    ("<Shift-a>", "Toggle admin-only"),
                    ("<↑/↓>", "Select"),
                    ("</>", "Search"),
                ],
                &keymap[..],
            ]
            .concat();
            if filter.is_admin_only() {
                keymap.push(("<Ctrl-d>", "Delete"));
                keymap.push(("<i>", "Invite"));
                keymap.push(("<r>", "Remove"));
            }
        }
        View::Apps { .. } => {
            keymap = [
                &[
                    ("<Enter>", "List machines"),
                    ("<o>", "Open"),
                    ("<l>", "Logs"),
                    ("<v>", "View releases"),
                    ("<s>", "View services"),
                    ("<Ctrl-r>", "Restart"),
                    ("<Ctrl-d>", "Destroy"),
                    ("<↑/↓>", "Select"),
                    ("</>", "Search"),
                    ("<Space>", "Toggle checkbox"),
                ],
                &keymap[..],
            ]
            .concat();
        }
        View::Machines { .. } => {
            keymap = [
                &[
                    ("<Enter>, <l>", "Logs"),
                    ("<r>", "Restart"),
                    ("<s>", "Start"),
                    ("<u>", "Suspend"),
                    ("<t>", "Stop"),
                    ("<Ctrl-k>", "Kill"),
                    ("<Ctrl-d>", "Destroy"),
                    ("<c>", "Cordon"),
                    ("<Shift-c>", "Uncordon"),
                    ("<↑/↓>", "Select"),
                    ("</>", "Search"),
                    ("<Space>", "Toggle checkbox"),
                ],
                &keymap[..],
            ]
            .concat();
        }
        View::Volumes { .. } => {
            keymap = [
                &[
                    ("<Ctrl-d>", "Destroy"),
                    ("<↑/↓>", "Select"),
                    ("</>", "Search"),
                    ("<Space>", "Toggle checkbox"),
                ],
                &keymap[..],
            ]
            .concat();
        }
        View::Secrets { .. } => {
            keymap = [
                &[
                    ("<u>", "Stage Unset"),
                    ("<↑/↓>", "Select"),
                    ("</>", "Search"),
                    ("<Space>", "Toggle checkbox"),
                ],
                &keymap[..],
            ]
            .concat();
        }
        View::AppLogs { .. } => {
            keymap = [
                &[
                    ("<t>", "Toggle region selector"),
                    ("<↑/↓>", "Select region"),
                    ("<f>", "Focus region"),
                    ("<←/→>", "Change display filter level"),
                    ("<+/->", "Change filter level"),
                    ("<Ctrl-s>", "Dump logs"),
                    ("<PageUp/Down>", "Scroll"),
                    ("<r>", "Reset scroll"),
                ],
                &keymap[..],
            ]
            .concat();
        }
        View::MachineLogs { .. } => {
            keymap = [
                &[
                    ("<Ctrl-s>", "Dump logs"),
                    ("<PageUp/Down>", "Scroll"),
                    ("<r>", "Reset scroll"),
                ],
                &keymap[..],
            ]
            .concat();
        }
    }

    if matches!(state.multi_select_mode, MultiSelectMode::On(..)) {
        keymap = [&keymap[..], &[("<Enter>", "Apply")]].concat();
    }

    let max_item_width = keymap
        .iter()
        .map(|(key, action)| {
            key.len() + 2 + action.len() + 1 // +2 for ": " and + 1 for space at the end
        })
        .max()
        .unwrap_or(0);
    let available_width = header_layout[0].width as usize;
    let col_length = available_width / max_item_width;

    let keymap_columns_layout = Layout::default()
        .direction(Direction::Horizontal)
        .constraints(vec![Constraint::Min(0); col_length])
        .split(header_layout[0].inner(Margin::new(1, 1)));

    keymap_columns_layout
        .iter()
        .enumerate()
        .for_each(|(col_idx, &chunk)| {
            let row_length = keymap_columns_layout[col_idx].height as usize;
            let rows = Layout::default()
                .direction(Direction::Vertical)
                .constraints(vec![Constraint::Length(1); row_length])
                .split(chunk);

            keymap
                .iter()
                .skip(row_length * col_idx)
                .zip(rows.iter())
                .enumerate() // Add enumerate to track position
                .for_each(|(i, (&(key, action), row))| {
                    let multi_select_action = i + row_length * col_idx == (keymap.len() - 1);
                    let color = if matches!(state.multi_select_mode, MultiSelectMode::On(..))
                        && multi_select_action
                    {
                        Palette::TEAL
                    } else if let View::Organizations { ref filter } = &current_view {
                        let admin_only_actions = i + row_length * col_idx >= (keymap.len() - 3);
                        if filter.is_admin_only() && admin_only_actions {
                            Palette::BLUE
                        } else {
                            Palette::LIGHT_PURPLE
                        }
                    } else {
                        Palette::LIGHT_PURPLE
                    };

                    let line = Line::from(vec![
                        Span::styled(key, Style::default().fg(color)),
                        Span::raw(": "),
                        Span::raw(String::from(action) + " "),
                    ]);
                    frame.render_widget(Paragraph::new(line), *row);
                });
        });

    let banner_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(2)])
        .split(header_layout[1]);

    frame.render_widget(
        Block::default()
            .title(vec![
                "★ ".fg(Palette::TEAL),
                env!("CARGO_PKG_NAME").bold(),
                "-".fg(Color::White),
                env!("CARGO_PKG_VERSION").into(),
                " ★".fg(Palette::TEAL),
            ])
            .title_alignment(Alignment::Center),
        area,
    );

    let banner_logo = fly_balloon::FlyBalloonWidget::default();
    let banner_text = Paragraph::new("Manage your Fly.io resources")
        .centered()
        .wrap(Wrap { trim: true })
        .white();
    frame.render_widget(banner_logo, banner_layout[0]);
    frame.render_widget(banner_text, banner_layout[1]);
}

fn render_input_bar(state: &mut State, frame: &mut Frame, area: Rect) {
    let search_mode = matches!(state.input_state, InputState::Search { .. });
    let outer = Block::default()
        .borders(Borders::all())
        .border_style(Style::new().fg({
            if search_mode {
                Palette::BLUE
            } else {
                Palette::PINK
            }
        }));
    let outer_area = outer.inner(area);
    let layout = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Length(4), Constraint::Min(0)])
        .split(outer_area);

    frame.render_widget(outer, area);
    frame.render_widget(
        format!("{}> ", if search_mode { "🌞" } else { "🪁" }),
        layout[0],
    );

    let mut render_input = |input: &Input, content: Line| {
        let input_area = layout[1];
        let width = input_area.width.max(1) - 1; // keep 1 for cursor
        let scroll = input.visual_scroll(width as usize);
        let input_bar = Paragraph::new(content)
            .scroll((0, scroll as u16))
            .block(Block::default());

        frame.set_cursor_position((
            input_area.x + ((input.visual_cursor()).max(scroll) - scroll) as u16,
            input_area.y,
        ));
        frame.render_widget(input_bar, input_area);
    };

    match &state.input_state {
        InputState::Command { input, command } => {
            let mut input_text = vec![input.value().into()];
            input_text.push(command.strip_prefix(input.value()).unwrap_or("").dim());
            render_input(input, Line::from(input_text));
        }
        InputState::Search { input } => {
            render_input(input, Line::from(input.value()));
        }
        InputState::Hidden { .. } => {}
    }
}

/// Returns the line with the search result highlighted.
fn highlight_search_result<'a>(line: Line<'a>, input: &'a str) -> Vec<Span<'a>> {
    let line_str = line.to_string();
    if line_str.contains(input) && !input.is_empty() {
        let splits = line_str.split(input);
        let chunks = splits.into_iter().map(|c| Span::from(c.to_owned()));
        let pattern = Span::styled(input, Style::new().fg(Palette::BLUE).underlined());
        itertools::intersperse(chunks, pattern).collect::<Vec<Span>>()
    } else {
        line.spans.clone()
    }
}

fn render_current_view(state: &mut State, frame: &mut Frame, area: Rect) {
    let mut layout = vec![Constraint::Min(0), Constraint::Length(2)];

    let current_view = state.get_current_view();
    let is_multi_select_shown = matches!(state.multi_select_mode, MultiSelectMode::On(..))
        && matches!(
            current_view,
            View::Organizations { .. }
                | View::Apps { .. }
                | View::Machines { .. }
                | View::Volumes { .. }
                | View::Secrets { .. }
        );
    if is_multi_select_shown {
        layout.insert(0, Constraint::Length(2));
    }

    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints(layout)
        .split(area);

    let breadcrumbs = state.get_breadcrumbs();
    let breadcrumbs_layout = breadcrumbs
        .iter()
        .map(|text| Constraint::Length((text.width() + 3) as u16))
        .collect::<Vec<Constraint>>();

    let breadcrumbs_layout = Layout::default()
        .direction(Direction::Horizontal)
        .constraints(breadcrumbs_layout)
        .split(layout[layout.len() - 1]);

    breadcrumbs
        .iter()
        .zip(breadcrumbs_layout.iter())
        .for_each(|(breadcrumb, layout_chunk)| {
            let text = format!(" {} ", breadcrumb);
            let breadcrumb_widget = Paragraph::new(
                text.to_text()
                    .bold()
                    .bg(if current_view.to_breadcrumb().eq(breadcrumb) {
                        Palette::PINK
                    } else {
                        Palette::LIGHT_PURPLE
                    })
                    .fg(Palette::DARK_GRAY),
            )
            .wrap(Wrap { trim: false })
            .block(Block::default().padding(Padding::left(1)));

            frame.render_widget(breadcrumb_widget, *layout_chunk);
        });

    match current_view {
        View::Organizations { .. }
        | View::Apps { .. }
        | View::Machines { .. }
        | View::Volumes { .. }
        | View::Secrets { .. } => {
            if is_multi_select_shown {
                let multi_select_reason_feedback_text = match state.multi_select_mode {
                    MultiSelectMode::On(MultiSelectModeReason::RestartMachines) => {
                        "Select the machines you want to restart."
                    }
                    MultiSelectMode::On(MultiSelectModeReason::StartMachines) => {
                        "Select the machines you want to start."
                    }
                    MultiSelectMode::On(MultiSelectModeReason::SuspendMachines) => {
                        "Select the machines you want to suspend."
                    }
                    MultiSelectMode::On(MultiSelectModeReason::StopMachines) => {
                        "Select the machines you want to stop."
                    }
                    MultiSelectMode::On(MultiSelectModeReason::CordonMachines) => {
                        "Select the machines you want to deactivate the services on."
                    }
                    MultiSelectMode::On(MultiSelectModeReason::UncordonMachines) => {
                        "Select the machines you want to reactivate the services on."
                    }
                    MultiSelectMode::On(MultiSelectModeReason::UnsetSecrets) => {
                        "Select the secrets you want to stage unset."
                    }
                    _ => "",
                };
                let multi_select_reason_feedback_text = Paragraph::new(
                    multi_select_reason_feedback_text
                        .to_text()
                        .bold()
                        .fg(Palette::TEAL),
                )
                .wrap(Wrap { trim: false })
                .block(Block::default().padding(Padding::left(1)));
                frame.render_widget(multi_select_reason_feedback_text, layout[0]);
            }

            // Set the correct index for the selected resource
            let resource_list = &state.resource_list;
            let mut table_state = TableState::default();
            let selected_index = resource_list.state.selected();
            table_state.select(selected_index);

            let headers = current_view.headers();
            let max_cell_width = (layout[0].width as usize).saturating_sub(4) / headers.len();

            // Skip ids for apps and machines as we don't show them.
            let data_skip_index = match current_view {
                View::Organizations { .. } | View::Apps { .. } | View::Machines { .. } => 1,
                _ => 0,
            };

            let filtered_rows = resource_list.filtered_items.iter().map(|row| {
                let cells = row
                    .iter()
                    .skip(data_skip_index)
                    .enumerate()
                    .map(|(i, value)| {
                        let content = if value.width() > max_cell_width {
                            let truncated: String = value
                                .chars()
                                .take(max_cell_width.saturating_sub(3))
                                .collect();
                            format!("{}…", truncated)
                        } else {
                            value.clone()
                        };

                        let mut spans = if !resource_list.search_filter.is_empty() {
                            highlight_search_result(content.into(), &resource_list.search_filter)
                        } else {
                            Line::from(content).spans
                        };

                        if is_multi_select_shown && i == 0 {
                            let prefix = if resource_list.multi_select_state.contains(&row[0]) {
                                Span::from("[x] ").fg(Palette::TEAL)
                            } else {
                                Span::from("[ ] ")
                            };
                            spans.insert(0, prefix);
                        }

                        Cell::from(Line::from(spans))
                    });
                Row::new(cells)
            });

            let table = Table::new(
                filtered_rows,
                &[Constraint::Length(max_cell_width as u16)].repeat(headers.len()),
            )
            .header(Row::new(
                headers
                    .to_vec()
                    .iter()
                    .map(|v| Cell::from((*v).fg(Palette::LIGHT_PINK))),
            ))
            .column_spacing(0)
            .block(
                Block::default()
                    .title(Line::from({
                        let (is_view_orgs, is_admin_only) = match current_view {
                            View::Organizations { ref filter } => (true, filter.is_admin_only()),
                            _ => (false, false),
                        };
                        let scope_skip_index = if is_view_orgs { 0 } else { 1 };
                        let scopes = state.get_scopes().iter().skip(scope_skip_index).join("/");
                        let mut spans = vec![
                            Span::from(format!(" {}(", current_view))
                                .bold()
                                .fg(Palette::PINK),
                            Span::from(scopes)
                                .bold()
                                .fg(if is_view_orgs && is_admin_only {
                                    Palette::BLUE
                                } else {
                                    Palette::LIGHT_PURPLE
                                }),
                            Span::from(") ").bold().fg(Palette::PINK),
                        ];
                        if !resource_list.search_filter.is_empty() {
                            spans.push(Span::styled(
                                format!("/{}", resource_list.search_filter),
                                Style::default()
                                    .bg(Palette::DARK_GRAY)
                                    .fg(Palette::LIGHT_BLUE),
                            ));
                            spans.push(Span::raw(" "));
                        }
                        spans
                    }))
                    .title_alignment(Alignment::Center)
                    .borders(Borders::all())
                    .border_style(Style::new().fg({
                        if !resource_list.search_filter.is_empty() {
                            Palette::BLUE
                        } else if matches!(state.input_state, InputState::Command { .. }) {
                            Palette::PINK
                        } else {
                            Palette::PURPLE
                        }
                    }))
                    .padding(Padding::horizontal(1)),
            )
            .row_highlight_style(Style::default().bg(Palette::LIGHT_PURPLE).fg(Color::Black));
            frame.render_stateful_widget(
                table,
                layout[if is_multi_select_shown { 1 } else { 0 }],
                &mut table_state,
            );
        }
        View::AppLogs { .. } => {
            // info!("Logs opts: {:#?}", opts);
            let logs = TuiLoggerSmartWidget::default()
                .border_style(Style::new().fg({
                    // if !resource_list.search_filter.is_empty() {
                    //     Palette::BLUE
                    // }
                    if matches!(state.input_state, InputState::Command { .. }) {
                        Palette::PINK
                    } else {
                        Palette::PURPLE
                    }
                }))
                .highlight_style(Style::default().bg(Palette::DARK_PURPLE))
                .title_target(Line::from(" Regions ").fg(Palette::PINK))
                .title_log(Line::from({
                    let scopes = state.get_scopes().iter().skip(1).join("/");
                    let spans = vec![
                        Span::from(" App logs(").bold().fg(Palette::PINK),
                        Span::from(scopes).bold().fg(Palette::LIGHT_PURPLE),
                        Span::from(") ").bold().fg(Palette::PINK),
                    ];
                    // if !resource_list.search_filter.is_empty() {
                    //     spans.push(Span::styled(
                    //         format!("/{}", resource_list.search_filter),
                    //         Style::default()
                    //             .bg(Palette::DARK_GRAY)
                    //             .fg(Palette::LIGHT_BLUE),
                    //     ));
                    //     spans.push(Span::raw(" "));
                    // }
                    spans
                }))
                .style_error(Style::default().fg(Color::Red))
                .style_debug(Style::default().fg(Color::Green))
                .style_warn(Style::default().fg(Color::Yellow))
                .style_trace(Style::default().fg(Color::Magenta))
                .style_info(Style::default().fg(Color::Cyan))
                .output_separator(' ')
                .output_timestamp(Some("%H:%M:%S".to_string()))
                .output_level(Some(TuiLoggerLevelOutput::Long))
                .output_target(true)
                .output_file(false)
                .output_line(false)
                .state(&state.logs_state);

            frame.render_widget(logs, layout[0]);
        }
        View::MachineLogs { .. } => {
            // info!("Logs opts: {:#?}", opts);
            let logs = TuiLoggerWidget::default()
                .block(
                    Block::bordered()
                        .border_style(Style::new().fg({
                            // if !resource_list.search_filter.is_empty() {
                            //     Palette::BLUE
                            // }
                            if matches!(state.input_state, InputState::Command { .. }) {
                                Palette::PINK
                            } else {
                                Palette::PURPLE
                            }
                        }))
                        .title(Line::from({
                            let scopes = state.get_scopes().iter().skip(1).join("/");
                            let spans = vec![
                                Span::from(" Machine logs(").bold().fg(Palette::PINK),
                                Span::from(scopes).bold().fg(Palette::LIGHT_PURPLE),
                                Span::from(") ").bold().fg(Palette::PINK),
                            ];
                            // if !resource_list.search_filter.is_empty() {
                            //     spans.push(Span::styled(
                            //         format!("/{}", resource_list.search_filter),
                            //         Style::default()
                            //             .bg(Palette::DARK_GRAY)
                            //             .fg(Palette::LIGHT_BLUE),
                            //     ));
                            //     spans.push(Span::raw(" "));
                            // }
                            spans
                        })),
                )
                .style_error(Style::default().fg(Color::Red))
                .style_debug(Style::default().fg(Color::Green))
                .style_warn(Style::default().fg(Color::Yellow))
                .style_trace(Style::default().fg(Color::Magenta))
                .style_info(Style::default().fg(Color::Cyan))
                .output_separator(' ')
                .output_timestamp(Some("%H:%M:%S".to_string()))
                .output_level(Some(TuiLoggerLevelOutput::Long))
                .output_target(true)
                .output_file(false)
                .output_line(false)
                .state(&state.logs_state);

            frame.render_widget(logs, layout[0]);
        }
    }
}

fn render_radar_popup(state: &mut State, frame: &mut Frame, area: Rect) {
    let current_view = state.get_current_view();
    let popup_state = &state.popup;

    if let Some(popup_state) = popup_state {
        let (title, popup_actions_index) = match popup_state.popup_type {
            PopupType::DestroyResourcePopup => {
                let popup_actions_index = if matches!(current_view, View::Machines { .. }) {
                    1
                } else {
                    0
                };
                let title = match current_view {
                    View::Apps { .. } => "Destroy the app",
                    View::Machines { .. } => "Destroy the machine",
                    View::Volumes { .. } => "Destroy the volume",
                    View::Secrets { .. } => "Stage Unset the secret",
                    _ => "Destroy the resource",
                };
                (
                    Line::from(vec![
                        "🗑️ ".to_span(),
                        title.fg(Color::LightBlue).bold(),
                        " 🗑️".to_span(),
                    ]),
                    popup_actions_index,
                )
            }
            PopupType::RestartResourcePopup => {
                let title = match current_view {
                    View::Apps { .. } => "Restart the app",
                    View::Machines { .. } => "Restart the machines",
                    _ => "Restart the resource",
                };
                (
                    Line::from(vec![
                        "🔁 ".to_span(),
                        title.fg(Color::LightCyan).bold(),
                        " 🔁".to_span(),
                    ]),
                    1,
                )
            }
            PopupType::ErrorPopup => (
                Line::from(vec![
                    "⛈️ ".to_span(),
                    "Error".fg(Color::Red).bold(),
                    " ⛈️".to_span(),
                ]),
                0,
            ),
            PopupType::InfoPopup => (
                Line::from(vec![
                    "ℹ️ ".to_span(),
                    "Info".fg(Color::LightGreen).bold(),
                    " ℹ️".to_span(),
                ]),
                0,
            ),
            PopupType::ViewAppReleasesPopup => (
                Line::from(vec![
                    "🤖 ".to_span(),
                    "App releases".fg(Palette::PINK).bold(),
                    " 🤖".to_span(),
                ]),
                0,
            ),
            PopupType::ViewAppServicesPopup => (
                Line::from(vec![
                    "🌟 ".to_span(),
                    "App services".fg(Color::Yellow).bold(),
                    " 🌟".to_span(),
                ]),
                0,
            ),
            PopupType::ViewCommandsPopup => (
                Line::from(vec![
                    "🪁 ".to_span(),
                    "Commands".fg(Palette::PINK).bold(),
                    " 🪁".to_span(),
                ]),
                0,
            ),
            PopupType::StartMachinesPopup => (
                Line::from(vec![
                    "▶️ ".to_span(),
                    "Start machines".fg(Palette::LIGHT_PINK).bold(),
                    " ▶️".to_span(),
                ]),
                0,
            ),
            PopupType::SuspendMachinesPopup => (
                Line::from(vec![
                    "💤 ".to_span(),
                    "Suspend machines".fg(Palette::DARK_BLUE).bold(),
                    " 💤".to_span(),
                ]),
                0,
            ),
            PopupType::StopMachinesPopup => (
                Line::from(vec![
                    "⏹️ ".to_span(),
                    "Stop machines".fg(Palette::DARK_PINK).bold(),
                    " ⏹️".to_span(),
                ]),
                0,
            ),
            PopupType::KillMachinePopup => (
                Line::from(vec![
                    "🛑 ".to_span(),
                    "Kill the machine".fg(Color::Red).bold(),
                    " 🛑".to_span(),
                ]),
                0,
            ),
            PopupType::CordonMachinesPopup => (
                Line::from(vec![
                    "🚧 ".to_span(),
                    "Cordon machines".fg(Palette::TEAL).bold(),
                    " 🚧".to_span(),
                ]),
                0,
            ),
            PopupType::UncordonMachinesPopup => (
                Line::from(vec![
                    "🆓 ".to_span(),
                    "Uncordon machines".fg(Palette::TEAL).bold(),
                    " 🆓".to_span(),
                ]),
                0,
            ),
        };
        let popup = Block::default()
            .title(title.alignment(Alignment::Center))
            .style(Style::default().white().on_black())
            .borders(Borders::ALL)
            .border_set(border::ROUNDED)
            .border_style(Style::new().bold().fg(Palette::PURPLE));

        let (op_actions, popup_actions) =
            popup_state.actions.children.split_at(popup_actions_index);
        let op_actions: Vec<&CheckBox> = op_actions
            .iter()
            .filter_map(|action| action.as_any().downcast_ref::<CheckBox>())
            .collect();
        let popup_actions: Vec<&TextBox> = popup_actions
            .iter()
            .filter_map(|action| action.as_any().downcast_ref::<TextBox>())
            .collect();

        if matches!(popup_state.popup_type, PopupType::ViewAppReleasesPopup) {
            let percent_x = 100;
            let percent_y = 75;
            let headers = [
                "Version",
                "Status",
                "Description",
                "User",
                "Date",
                "Docker Image",
            ];
            let mut max_cell_widths = vec![10, 10, 12, 12, 20];
            // INFO: calc width based on percent_x, then - padding 2, border 2 then - sum of preceding
            // cell widths
            let last_col_max_cell_width = ((area.width as usize) * percent_x / 100_usize)
                .saturating_sub(4)
                .saturating_sub(max_cell_widths.iter().sum());
            max_cell_widths.push(last_col_max_cell_width);

            let app_releases_list = &state.app_releases_list.clone();
            let rows = app_releases_list.iter().map(|row| {
                let cells = row.iter().enumerate().map(|(i, value)| {
                    let max_cell_width = max_cell_widths[i];
                    let content = if value.width() > max_cell_width {
                        let truncated: String = value
                            .chars()
                            .take(max_cell_width.saturating_sub(3))
                            .collect();
                        format!("{}…", truncated)
                    } else {
                        value.clone()
                    };
                    Cell::from(Line::from(content))
                });
                Row::new(cells)
            });

            let content = Table::new(
                rows,
                max_cell_widths
                    .iter()
                    .map(|w| Constraint::Length(*w as u16)),
            )
            .header(Row::new(
                headers
                    .to_vec()
                    .iter()
                    .map(|v| Cell::from((*v).fg(Palette::LIGHT_PINK).bold())),
            ))
            .column_spacing(0)
            .block(
                Block::default()
                    .title(
                        Line::from(Span::from(&popup_state.message))
                            .bold()
                            .fg(Palette::LIGHT_PURPLE),
                    )
                    .title_alignment(Alignment::Center)
                    .padding(Padding::vertical(1)),
            );

            render_popup(
                frame,
                area,
                percent_x as u16,
                percent_y as u16,
                popup,
                content,
                op_actions,
                popup_actions,
            );
        } else if matches!(popup_state.popup_type, PopupType::ViewAppServicesPopup) {
            let percent_x = 100;
            let percent_y = 75;
            let headers = [
                "Protocol",
                "Ports",
                "Handlers",
                "Force Https",
                "Process Group",
                "Regions",
                "Machines",
            ];
            // INFO: calc width based on percent_x, then - padding 2, border 2
            let max_cell_width =
                ((area.width as usize) * percent_x / 100_usize).saturating_sub(4) / headers.len();
            let app_services_list = &state.app_services_list.clone();
            let rows = app_services_list.iter().map(|row| {
                let cells = row.iter().map(|value| {
                    let content = if value.width() > max_cell_width {
                        let truncated: String = value
                            .chars()
                            .take(max_cell_width.saturating_sub(3))
                            .collect();
                        format!("{}…", truncated)
                    } else {
                        value.clone()
                    };
                    Cell::from(Line::from(content))
                });
                Row::new(cells)
            });

            let content = Table::new(
                rows,
                &[Constraint::Length(max_cell_width as u16)].repeat(headers.len()),
            )
            .header(Row::new(
                headers
                    .to_vec()
                    .iter()
                    .map(|v| Cell::from((*v).fg(Palette::LIGHT_PINK).bold())),
            ))
            .column_spacing(0)
            .block(
                Block::default()
                    .title(
                        Line::from(Span::from(&popup_state.message))
                            .bold()
                            .fg(Palette::LIGHT_PURPLE),
                    )
                    .title_alignment(Alignment::Center)
                    .padding(Padding::vertical(1)),
            );

            render_popup(
                frame,
                area,
                percent_x as u16,
                percent_y as u16,
                popup,
                content,
                op_actions,
                popup_actions,
            );
        } else if matches!(popup_state.popup_type, PopupType::ViewCommandsPopup) {
            let percent_x = 100;
            let percent_y = 75;
            let headers = ["Name", "Aliases"];
            let commands_list = COMMANDS
                .iter()
                .filter_map(|&cmd_str| {
                    cmd_str
                        .parse::<Command>()
                        .ok()
                        .map(|cmd| vec![cmd_str.to_string(), cmd.to_aliases().join(", ")])
                })
                .collect::<Vec<Vec<String>>>();
            // INFO: calc width based on percent_x, then - padding 2, border 2
            let max_cell_width =
                ((area.width as usize) * percent_x / 100_usize).saturating_sub(4) / headers.len();

            let rows = commands_list.iter().map(|row| {
                let cells = row.iter().map(|value| {
                    let content = if value.width() > max_cell_width {
                        let truncated: String = value
                            .chars()
                            .take(max_cell_width.saturating_sub(3))
                            .collect();
                        format!("{}…", truncated)
                    } else {
                        value.clone()
                    };
                    Cell::from(Line::from(content))
                });
                Row::new(cells)
            });

            let content = Table::new(
                rows,
                &[Constraint::Length(max_cell_width as u16)].repeat(headers.len()),
            )
            .header(Row::new(
                headers
                    .to_vec()
                    .iter()
                    .map(|v| Cell::from((*v).fg(Palette::LIGHT_PINK).bold())),
            ))
            .column_spacing(0);

            render_popup(
                frame,
                area,
                percent_x as u16,
                percent_y as u16,
                popup,
                content,
                op_actions,
                popup_actions,
            );
        } else {
            let percent_x = 50;
            let percent_y = 30;
            //INFO: calc width based on percent_x and then - padding 2, border 2
            let mut max_line_width = (area.width as usize) * percent_x / 100_usize;
            max_line_width = max_line_width.saturating_sub(4);

            let lines = [popup_state.message.to_string()];
            let lines: Vec<Line> = lines
                .into_iter()
                .flat_map(|v| {
                    if v.width() > max_line_width {
                        textwrap::wrap(&v, textwrap::Options::new(max_line_width))
                            .into_iter()
                            .map(|v| Line::from(v.to_string()))
                            .collect()
                    } else {
                        vec![Line::from(v)]
                    }
                })
                .collect();
            let content = Text::from(lines);

            render_popup(
                frame,
                area,
                percent_x as u16,
                percent_y as u16,
                popup,
                content,
                op_actions,
                popup_actions,
            );
        }
    }
}

/// Renders the user interface widgets.
pub fn render(state: &mut State, frame: &mut Frame) {
    if state.splash_shown.load(Ordering::SeqCst) {
        let mut main_layout = vec![Constraint::Min(0)];
        if cfg!(debug_assertions) {
            main_layout.push(Constraint::Percentage(40));
        }
        let main_layout = Layout::horizontal(main_layout).split(frame.area());
        let mut layout = vec![Constraint::Length(8), Constraint::Min(0)];
        if !matches!(state.input_state, InputState::Hidden { .. }) {
            layout.insert(1, Constraint::Length(3));
        }
        let outer = Block::default().bg(Color::Black);
        let outer_area = outer.inner(frame.area());
        frame.render_widget(outer, frame.area());

        let layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints(layout)
            // .split(outer_area);
            .split(main_layout[0]);

        #[cfg(debug_assertions)]
        render_debugger(state, frame, main_layout[1]);

        render_header(state, frame, layout[0]);
        if !matches!(state.input_state, InputState::Hidden { .. }) {
            render_input_bar(state, frame, layout[1]);
        }
        render_current_view(state, frame, layout.last().unwrap().to_owned());
        render_radar_popup(state, frame, outer_area);
    } else {
        render_splash(frame);
    }
}

#[cfg(debug_assertions)]
fn render_debugger(state: &mut State, frame: &mut Frame, area: Rect) {
    let logger = tui_logger::TuiLoggerWidget::default()
        .block(Block::bordered().title("Debugger"))
        .output_separator('|')
        .output_timestamp(Some("%F %H:%M:%S%.3f".to_string()))
        .output_level(Some(tui_logger::TuiLoggerLevelOutput::Long))
        .output_target(false)
        .output_file(false)
        .output_line(false)
        .style(Style::default().fg(Color::White))
        .state(&state.debugger_state);

    frame.render_widget(logger, area);
}
