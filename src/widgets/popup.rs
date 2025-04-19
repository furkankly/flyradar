use std::iter::zip;

use ratatui::layout::{Constraint, Direction, Flex, Layout, Rect};
use ratatui::style::Style;
use ratatui::text::Line;
use ratatui::widgets::{Block, Borders, Clear, Padding, Widget};
use ratatui::Frame;
use tracing::info;
use tui_input::Input;
use unicode_width::UnicodeWidthStr;

use super::focusable_check_box::CheckBox;
use super::focusable_text::TextBox;
use crate::ui::{render_input, Palette};

pub fn render_popup<C: Widget>(
    frame: &mut Frame,
    area: Rect,
    percent_x: u16,
    percent_y: u16,
    popup: Block,
    main_content: C,
    input: Option<&Input>,
    input_label: String,
    op_actions: Vec<&CheckBox>,
    popup_actions: Vec<&TextBox>,
) {
    let area = popup_area(area, percent_x, percent_y);
    let popup = popup.padding(Padding::uniform(1));
    let popup_area = popup.inner(area);
    frame.render_widget(Clear, area); //this clears out the background
    frame.render_widget(popup, area);
    let layout =
        Layout::vertical(vec![Constraint::Min(0), Constraint::Length(1)]).split(popup_area);
    let mut content_layout = vec![Constraint::Min(0)];
    if input.is_some() {
        content_layout.insert(1, Constraint::Length(3));
    }
    if !op_actions.is_empty() {
        content_layout.insert(content_layout.len() - 1, Constraint::Min(0));
    }
    let content_layout = Layout::vertical(content_layout).split(layout[0]);
    frame.render_widget(main_content, content_layout[0]);
    info!("layout: {:#?}", content_layout);
    if let Some(input) = &input {
        let outer = Block::default()
            .borders(Borders::all())
            .border_style(Style::new().fg(Palette::BLUE));
        let outer_area = outer.inner(content_layout[1]);
        frame.render_widget(outer, content_layout[1]);
        let input_layout = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Length(input_label.width() as u16),
                Constraint::Min(0),
            ])
            .split(outer_area);

        frame.render_widget(input_label, input_layout[0]);
        render_input(frame, input_layout[1], input, Line::from(input.value()));
    };
    if !op_actions.is_empty() {
        render_op_actions(frame, content_layout[content_layout.len() - 1], op_actions);
    }
    render_popup_actions(frame, layout[layout.len() - 1], popup_actions);
}

fn popup_area(area: Rect, percent_x: u16, percent_y: u16) -> Rect {
    let vertical = Layout::vertical([Constraint::Percentage(percent_y)]).flex(Flex::Center);
    let horizontal = Layout::horizontal([Constraint::Percentage(percent_x)]).flex(Flex::Center);
    let [area] = vertical.areas(area);
    let [area] = horizontal.areas(area);
    area
}

fn render_op_actions(frame: &mut Frame, area: Rect, actions: Vec<&CheckBox>) {
    let mut layout = vec![];
    actions.iter().for_each(|action| {
        let width = action.content.width();
        layout.push(Constraint::Length((width + 2) as u16));
    });
    let areas = Layout::vertical(layout).flex(Flex::Center).split(area);

    zip(actions.iter(), areas.iter())
        .for_each(|(&action, &area)| frame.render_widget(action, area));
}

fn render_popup_actions(frame: &mut Frame, area: Rect, actions: Vec<&TextBox>) {
    let mut layout = vec![Constraint::Min(0)];
    actions.iter().for_each(|action| {
        let width = action.content.width();
        layout.push(Constraint::Length((width + 2) as u16));
    });
    let areas = Layout::horizontal(layout).flex(Flex::End).split(area);

    zip(actions.iter(), areas.iter().skip(1))
        .for_each(|(&action, &area)| frame.render_widget(action, area));
}
