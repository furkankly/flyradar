use std::any::Any;

use focusable::Focus;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Style, Stylize};
use ratatui::text::{Line, Span};
use ratatui::widgets::WidgetRef;

use super::focusable_widget::FocusableWidget;
use crate::ui::Palette;

#[derive(Debug, Clone, Focus)]
pub struct CheckBox {
    pub is_focused: bool,
    pub content: String,
    pub is_checked: bool,
}

impl CheckBox {
    pub fn new(content: &str, is_checked: bool) -> Self {
        Self {
            is_focused: false,
            content: content.to_string(),
            is_checked,
        }
    }

    pub fn toggle(&mut self) {
        self.is_checked = !self.is_checked;
    }

    pub fn is_check(&self) -> bool {
        self.is_checked
    }
}

impl FocusableWidget for CheckBox {
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

impl WidgetRef for CheckBox {
    fn render_ref(&self, area: Rect, buf: &mut Buffer) {
        let style = if self.is_focused {
            Style::new().bg(Palette::LIGHT_PURPLE).underlined().bold()
        } else {
            Style::new().white().on_black()
        };

        let checkbox = if self.is_checked {
            Span::styled("[x] ", Style::new().fg(Palette::DARK_TEAL))
        } else {
            Span::styled("[ ] ", Style::new())
        };

        let content = Span::styled(&self.content, style);
        let check_box = Line::from(vec![checkbox, content]);
        check_box.render_ref(area, buf);
    }
}
