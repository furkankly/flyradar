use std::iter::zip;

use ratatui::layout::{Constraint, Flex, Layout, Rect};
use ratatui::widgets::{Block, Clear, Padding, Widget};
use ratatui::Frame;
use unicode_width::UnicodeWidthStr;

use super::focusable_check_box::CheckBox;
use super::focusable_text::TextBox;

pub fn render_popup<C: Widget>(
    frame: &mut Frame,
    area: Rect,
    percent_x: u16,
    percent_y: u16,
    popup: Block,
    content: C,
    op_actions: Vec<&CheckBox>,
    popup_actions: Vec<&TextBox>,
) {
    let area = popup_area(area, percent_x, percent_y);
    let popup = popup.padding(Padding::uniform(1));
    let popup_area = popup.inner(area);
    frame.render_widget(Clear, area); //this clears out the background
    frame.render_widget(popup, area);
    let mut layout = vec![Constraint::Min(0), Constraint::Length(1)];
    if !op_actions.is_empty() {
        layout.insert(1, Constraint::Min(0));
    }
    let layout = Layout::vertical(layout).split(popup_area);
    frame.render_widget(content, layout[0]);
    if !op_actions.is_empty() {
        render_op_actions(frame, layout[1], op_actions);
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
