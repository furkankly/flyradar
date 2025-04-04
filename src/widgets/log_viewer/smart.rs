use std::sync::Arc;

use parking_lot::Mutex;
use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::Style;
use ratatui::text::Line;
use ratatui::widgets::{Block, BorderType, Borders, Padding, Widget};

use super::inner::TuiWidgetInnerState;
use super::standard::TuiLoggerWidget;
use crate::widgets::log_viewer::{
    LevelFilter, TuiLoggerLevelOutput, TuiLoggerTargetWidget, TuiWidgetState, TUI_LOGGER,
};

/// The Smart Widget combines the TuiLoggerWidget and the TuiLoggerTargetWidget
/// into a nice combo, where the TuiLoggerTargetWidget can be shown/hidden.
///
/// In the title the number of logging messages/s in the whole buffer is shown.
pub struct TuiLoggerSmartWidget<'a> {
    title_log: Line<'a>,
    title_target: Line<'a>,
    style: Option<Style>,
    border_style: Style,
    border_type: BorderType,
    highlight_style: Option<Style>,
    style_error: Option<Style>,
    style_warn: Option<Style>,
    style_debug: Option<Style>,
    style_trace: Option<Style>,
    style_info: Option<Style>,
    style_show: Option<Style>,
    style_hide: Option<Style>,
    style_off: Option<Style>,
    format_separator: Option<char>,
    format_timestamp: Option<Option<String>>,
    format_output_level: Option<Option<TuiLoggerLevelOutput>>,
    format_output_target: Option<bool>,
    format_output_file: Option<bool>,
    format_output_line: Option<bool>,
    state: Arc<Mutex<TuiWidgetInnerState>>,
}
impl Default for TuiLoggerSmartWidget<'_> {
    fn default() -> Self {
        //TUI_LOGGER.move_events();
        TuiLoggerSmartWidget {
            title_log: Line::from("Tui Log"),
            title_target: Line::from("Tui Target Selector"),
            style: None,
            border_style: Style::default(),
            border_type: BorderType::Plain,
            highlight_style: None,
            style_error: None,
            style_warn: None,
            style_debug: None,
            style_trace: None,
            style_info: None,
            style_show: None,
            style_hide: None,
            style_off: None,
            format_separator: None,
            format_timestamp: None,
            format_output_level: None,
            format_output_target: None,
            format_output_file: None,
            format_output_line: None,
            state: Arc::new(Mutex::new(TuiWidgetInnerState::new())),
        }
    }
}
impl<'a> TuiLoggerSmartWidget<'a> {
    pub fn highlight_style(mut self, style: Style) -> Self {
        self.highlight_style = Some(style);
        self
    }
    pub fn border_style(mut self, style: Style) -> Self {
        self.border_style = style;
        self
    }
    pub fn border_type(mut self, border_type: BorderType) -> Self {
        self.border_type = border_type;
        self
    }
    pub fn style(mut self, style: Style) -> Self {
        self.style = Some(style);
        self
    }
    pub fn style_error(mut self, style: Style) -> Self {
        self.style_error = Some(style);
        self
    }
    pub fn style_warn(mut self, style: Style) -> Self {
        self.style_warn = Some(style);
        self
    }
    pub fn style_info(mut self, style: Style) -> Self {
        self.style_info = Some(style);
        self
    }
    pub fn style_trace(mut self, style: Style) -> Self {
        self.style_trace = Some(style);
        self
    }
    pub fn style_debug(mut self, style: Style) -> Self {
        self.style_debug = Some(style);
        self
    }
    pub fn style_off(mut self, style: Style) -> Self {
        self.style_off = Some(style);
        self
    }
    pub fn style_hide(mut self, style: Style) -> Self {
        self.style_hide = Some(style);
        self
    }
    pub fn style_show(mut self, style: Style) -> Self {
        self.style_show = Some(style);
        self
    }
    /// Separator character between field.
    /// Default is ':'
    pub fn output_separator(mut self, sep: char) -> Self {
        self.format_separator = Some(sep);
        self
    }
    /// The format string can be defined as described in
    /// <https://docs.rs/chrono/0.4.19/chrono/format/strftime/index.html>
    ///
    /// If called with None, timestamp is not included in output.
    ///
    /// Default is %H:%M:%S
    pub fn output_timestamp(mut self, fmt: Option<String>) -> Self {
        self.format_timestamp = Some(fmt);
        self
    }
    /// Possible values are
    /// - TuiLoggerLevelOutput::Long        => DEBUG/TRACE/...
    /// - TuiLoggerLevelOutput::Abbreviated => D/T/...
    ///
    /// If called with None, level is not included in output.
    ///
    /// Default is Long
    pub fn output_level(mut self, level: Option<TuiLoggerLevelOutput>) -> Self {
        self.format_output_level = Some(level);
        self
    }
    /// Enables output of target field of event
    ///
    /// Default is true
    pub fn output_target(mut self, enabled: bool) -> Self {
        self.format_output_target = Some(enabled);
        self
    }
    /// Enables output of file field of event
    ///
    /// Default is true
    pub fn output_file(mut self, enabled: bool) -> Self {
        self.format_output_file = Some(enabled);
        self
    }
    /// Enables output of line field of event
    ///
    /// Default is true
    pub fn output_line(mut self, enabled: bool) -> Self {
        self.format_output_line = Some(enabled);
        self
    }
    pub fn title_target<T>(mut self, title: T) -> Self
    where
        T: Into<Line<'a>>,
    {
        self.title_target = title.into();
        self
    }
    pub fn title_log<T>(mut self, title: T) -> Self
    where
        T: Into<Line<'a>>,
    {
        self.title_log = title.into();
        self
    }
    pub fn state(mut self, state: &TuiWidgetState) -> Self {
        self.state = state.inner.clone();
        self
    }
}
impl Widget for TuiLoggerSmartWidget<'_> {
    /// Nothing to draw for combo widget
    fn render(self, area: Rect, buf: &mut Buffer) {
        let entries_s = {
            let tui_lock = TUI_LOGGER.inner.lock();
            let first_timestamp = tui_lock
                .events
                .iter()
                .next()
                .map(|entry| entry.timestamp.timestamp_millis());
            let last_timestamp = tui_lock
                .events
                .rev_iter()
                .next()
                .map(|entry| entry.timestamp.timestamp_millis());
            if let Some(first) = first_timestamp {
                if let Some(last) = last_timestamp {
                    // info!("first: {} last: {} dt: {}", first, last, last - first);
                    let dt = last - first;
                    let min_time_diff = 8000; // 8 seconds in ms
                    if dt >= min_time_diff {
                        tui_lock.events.len() as f64 / (dt as f64) * 1000.0
                    } else {
                        0.0
                    }
                } else {
                    0.0
                }
            } else {
                0.0
            }
        };

        let mut title_log = self.title_log.clone();
        title_log
            .spans
            .push(format!(" [log={:.1}/s]", entries_s).into());

        let hide_target = self.state.lock().hide_target;
        if hide_target {
            let tui_lw = TuiLoggerWidget::default()
                .block(
                    Block::default()
                        .title(title_log)
                        .border_style(self.border_style)
                        .border_type(self.border_type)
                        .borders(Borders::ALL),
                )
                .opt_style(self.style)
                .opt_style_error(self.style_error)
                .opt_style_warn(self.style_warn)
                .opt_style_info(self.style_info)
                .opt_style_debug(self.style_debug)
                .opt_style_trace(self.style_trace)
                .opt_output_separator(self.format_separator)
                .opt_output_timestamp(self.format_timestamp)
                .opt_output_level(self.format_output_level)
                .opt_output_target(self.format_output_target)
                .opt_output_file(self.format_output_file)
                .opt_output_line(self.format_output_line)
                .inner_state(self.state);
            tui_lw.render(area, buf);
        } else {
            let mut width: usize = 0;
            {
                let hot_targets = &TUI_LOGGER.inner.lock().targets;
                let mut state = self.state.lock();
                let hide_off = state.hide_off;
                {
                    let targets = &mut state.config;
                    targets.merge(hot_targets);
                    for (t, levelfilter) in targets.iter() {
                        if hide_off && levelfilter == &LevelFilter::Off {
                            continue;
                        }
                        width = width.max(t.len())
                    }
                }
            }
            // for horizontal padding of 1
            width += 2;
            let chunks = Layout::default()
                .direction(Direction::Horizontal)
                .constraints(vec![
                    Constraint::Length(width as u16 + 6 + 2),
                    Constraint::Min(10),
                ])
                .split(area);
            let tui_ltw = TuiLoggerTargetWidget::default()
                .block(
                    Block::default()
                        .padding(Padding::horizontal(1))
                        .title(self.title_target)
                        .border_style(self.border_style)
                        .border_type(self.border_type)
                        .borders(Borders::ALL),
                )
                .opt_style(self.style)
                .opt_highlight_style(self.highlight_style)
                .opt_style_off(self.style_off)
                .opt_style_hide(self.style_hide)
                .opt_style_show(self.style_show)
                .inner_state(self.state.clone());
            tui_ltw.render(chunks[0], buf);
            let tui_lw = TuiLoggerWidget::default()
                .block(
                    Block::default()
                        .title(title_log)
                        .border_style(self.border_style)
                        .border_type(self.border_type)
                        .borders(Borders::ALL),
                )
                .opt_style(self.style)
                .opt_style_error(self.style_error)
                .opt_style_warn(self.style_warn)
                .opt_style_info(self.style_info)
                .opt_style_debug(self.style_debug)
                .opt_style_trace(self.style_trace)
                .opt_output_separator(self.format_separator)
                .opt_output_timestamp(self.format_timestamp)
                .opt_output_level(self.format_output_level)
                .opt_output_target(self.format_output_target)
                .opt_output_file(self.format_output_file)
                .opt_output_line(self.format_output_line)
                .inner_state(self.state.clone());
            tui_lw.render(chunks[1], buf);
        }
    }
}
