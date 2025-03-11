use std::any::Any;

use focusable::Focus;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Style, Stylize};
use ratatui::text::Span;
use ratatui::widgets::WidgetRef;

use super::focusable_widget::FocusableWidget;
use crate::ui::Palette;

#[derive(Debug, Clone, Focus)]
pub struct TextBox {
    pub is_focused: bool,
    pub content: String,
}

impl TextBox {
    pub fn new(content: &str) -> Self {
        Self {
            is_focused: false,
            content: content.to_string(),
        }
    }
}

impl FocusableWidget for TextBox {
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

impl WidgetRef for TextBox {
    fn render_ref(&self, area: Rect, buf: &mut Buffer) {
        let style = if self.is_focused {
            Style::new().bg(Palette::LIGHT_PURPLE).underlined().bold()
        } else {
            Style::new().white().on_black()
        };
        Span::styled(&self.content, style).render_ref(area, buf);
    }
}
