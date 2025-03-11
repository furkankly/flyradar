use std::sync::Arc;

use chrono::Local;
use parking_lot::Mutex;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::widgets::{Block, Widget};

use super::inner::TuiWidgetInnerState;
use crate::widgets::log_viewer::{
    CircularBuffer, ExtLogRecord, Level, TuiLoggerLevelOutput, TuiWidgetState, TUI_LOGGER,
};

pub struct TuiLoggerWidget<'b> {
    block: Option<Block<'b>>,
    /// Base style of the widget
    style: Style,
    /// Level based style
    style_error: Option<Style>,
    style_warn: Option<Style>,
    style_debug: Option<Style>,
    style_trace: Option<Style>,
    style_info: Option<Style>,
    format_separator: char,
    format_timestamp: Option<String>,
    format_output_level: Option<TuiLoggerLevelOutput>,
    format_output_target: bool,
    format_output_file: bool,
    format_output_line: bool,
    state: Arc<Mutex<TuiWidgetInnerState>>,
}
impl<'b> Default for TuiLoggerWidget<'b> {
    fn default() -> TuiLoggerWidget<'b> {
        //TUI_LOGGER.move_events();
        TuiLoggerWidget {
            block: None,
            style: Default::default(),
            style_error: None,
            style_warn: None,
            style_debug: None,
            style_trace: None,
            style_info: None,
            format_separator: ':',
            format_timestamp: Some("%H:%M:%S".to_string()),
            format_output_level: Some(TuiLoggerLevelOutput::Long),
            format_output_target: true,
            format_output_file: true,
            format_output_line: true,
            state: Arc::new(Mutex::new(TuiWidgetInnerState::new())),
        }
    }
}
impl<'b> TuiLoggerWidget<'b> {
    pub fn block(mut self, block: Block<'b>) -> Self {
        self.block = Some(block);
        self
    }
    pub fn opt_style(mut self, style: Option<Style>) -> Self {
        if let Some(s) = style {
            self.style = s;
        }
        self
    }
    pub fn opt_style_error(mut self, style: Option<Style>) -> Self {
        if style.is_some() {
            self.style_error = style;
        }
        self
    }
    pub fn opt_style_warn(mut self, style: Option<Style>) -> Self {
        if style.is_some() {
            self.style_warn = style;
        }
        self
    }
    pub fn opt_style_info(mut self, style: Option<Style>) -> Self {
        if style.is_some() {
            self.style_info = style;
        }
        self
    }
    pub fn opt_style_trace(mut self, style: Option<Style>) -> Self {
        if style.is_some() {
            self.style_trace = style;
        }
        self
    }
    pub fn opt_style_debug(mut self, style: Option<Style>) -> Self {
        if style.is_some() {
            self.style_debug = style;
        }
        self
    }
    pub fn style(mut self, style: Style) -> Self {
        self.style = style;
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
    pub fn opt_output_separator(mut self, opt_sep: Option<char>) -> Self {
        if let Some(ch) = opt_sep {
            self.format_separator = ch;
        }
        self
    }
    /// Separator character between field.
    /// Default is ':'
    pub fn output_separator(mut self, sep: char) -> Self {
        self.format_separator = sep;
        self
    }
    pub fn opt_output_timestamp(mut self, opt_fmt: Option<Option<String>>) -> Self {
        if let Some(fmt) = opt_fmt {
            self.format_timestamp = fmt;
        }
        self
    }
    /// The format string can be defined as described in
    /// <https://docs.rs/chrono/0.4.19/chrono/format/strftime/index.html>
    ///
    /// If called with None, timestamp is not included in output.
    ///
    /// Default is %H:%M:%S
    pub fn output_timestamp(mut self, fmt: Option<String>) -> Self {
        self.format_timestamp = fmt;
        self
    }
    pub fn opt_output_level(mut self, opt_fmt: Option<Option<TuiLoggerLevelOutput>>) -> Self {
        if let Some(fmt) = opt_fmt {
            self.format_output_level = fmt;
        }
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
        self.format_output_level = level;
        self
    }
    pub fn opt_output_target(mut self, opt_enabled: Option<bool>) -> Self {
        if let Some(enabled) = opt_enabled {
            self.format_output_target = enabled;
        }
        self
    }
    /// Enables output of target field of event
    ///
    /// Default is true
    pub fn output_target(mut self, enabled: bool) -> Self {
        self.format_output_target = enabled;
        self
    }
    pub fn opt_output_file(mut self, opt_enabled: Option<bool>) -> Self {
        if let Some(enabled) = opt_enabled {
            self.format_output_file = enabled;
        }
        self
    }
    /// Enables output of file field of event
    ///
    /// Default is true
    pub fn output_file(mut self, enabled: bool) -> Self {
        self.format_output_file = enabled;
        self
    }
    pub fn opt_output_line(mut self, opt_enabled: Option<bool>) -> Self {
        if let Some(enabled) = opt_enabled {
            self.format_output_line = enabled;
        }
        self
    }
    /// Enables output of line field of event
    ///
    /// Default is true
    pub fn output_line(mut self, enabled: bool) -> Self {
        self.format_output_line = enabled;
        self
    }
    pub fn inner_state(mut self, state: Arc<Mutex<TuiWidgetInnerState>>) -> Self {
        self.state = state;
        self
    }
    pub fn state(mut self, state: &TuiWidgetState) -> Self {
        self.state = state.inner.clone();
        self
    }
    fn format_event(&self, evt: &ExtLogRecord) -> (String, Option<Style>) {
        let mut output = String::new();
        let (col_style, lev_long, lev_abbr) = match evt.level {
            Level::Error => (self.style_error, "ERROR", "E"),
            Level::Warn => (self.style_warn, "WARN", "W"),
            Level::Info => (self.style_info, "INFO", "I"),
            Level::Debug => (self.style_debug, "DEBUG", "D"),
            Level::Trace => (self.style_trace, "TRACE", "T"),
        };
        if let Some(fmt) = self.format_timestamp.as_ref() {
            output.push_str(&format!(
                "{}",
                evt.timestamp.with_timezone(&Local).format(fmt)
            ));
            output.push(self.format_separator);
        }

        if !evt.meta.event.provider.is_empty() {
            if !evt.instance.is_empty() {
                output.push_str(&format!("{}[{}]", evt.meta.event.provider, evt.instance));
            } else {
                output.push_str(&evt.meta.event.provider);
            }
            output.push(self.format_separator);
        } else if !evt.instance.is_empty() {
            output.push_str(&evt.instance);
            output.push(self.format_separator);
        }

        if self.format_output_target {
            output.push_str(&evt.target);
            output.push(self.format_separator);
        }

        match &self.format_output_level {
            None => {}
            Some(TuiLoggerLevelOutput::Abbreviated) => {
                output.push_str(&format!("[{}]", lev_abbr));
                output.push(self.format_separator);
            }
            Some(TuiLoggerLevelOutput::Long) => {
                output.push_str(&format!("[{}]", lev_long));
                output.push(self.format_separator);
            }
        }

        if let Some(error) = &evt.meta.error {
            if error.code > 0 {
                output.push_str(&format!("error.code={}", error.code));
                output.push(self.format_separator);
            }
            if !error.message.is_empty() {
                output.push_str(&format!("error.message={}", error.message));
                output.push(self.format_separator);
            }
        }

        if let Some(http) = &evt.meta.http {
            output.push_str(&format!("request.method={}", http.request.method));
            output.push(self.format_separator);
        }
        if let Some(url) = &evt.meta.url {
            output.push_str(&format!("request.url={}", url.full));
            output.push(self.format_separator);
        }
        if let Some(http) = &evt.meta.http {
            output.push_str(&format!("request.id={}", http.request.id));
            output.push(self.format_separator);

            output.push_str(&format!("response.status={}", http.response.status_code));
            output.push(self.format_separator);
        }

        (output, col_style)
    }
}
impl Widget for TuiLoggerWidget<'_> {
    fn render(mut self, area: Rect, buf: &mut Buffer) {
        buf.set_style(area, self.style);
        let list_area = match self.block.take() {
            Some(b) => {
                let inner_area = b.inner(area);
                b.render(area, buf);
                inner_area
            }
            None => area,
        };
        let indent = 9;
        if list_area.width < indent + 4 || list_area.height < 1 {
            return;
        }

        let mut state = self.state.lock();
        let la_height = list_area.height as usize;
        let mut lines: Vec<(Option<Style>, u16, String)> = vec![];
        {
            state.opt_timestamp_next_page = None;
            let opt_timestamp_bottom = state.opt_timestamp_bottom;
            let mut opt_timestamp_prev_page = None;
            let tui_lock = TUI_LOGGER.inner.lock();
            let mut circular = CircularBuffer::new(10); // MAGIC constant
            for evt in tui_lock.events.rev_iter() {
                if let Some(level) = state.config.get(&evt.target) {
                    if level < evt.level {
                        continue;
                    }
                } else if let Some(level) = state.config.default_display_level {
                    if level < evt.level {
                        continue;
                    }
                }
                if state.focus_selected {
                    if let Some(target) = state.opt_selected_target.as_ref() {
                        if target != &evt.target {
                            continue;
                        }
                    }
                }
                // Here all filters have been applied,
                // So check, if user is paging through history
                if let Some(timestamp) = opt_timestamp_bottom.as_ref() {
                    if *timestamp < evt.timestamp {
                        circular.push(evt.timestamp);
                        continue;
                    }
                }
                if !circular.is_empty() {
                    state.opt_timestamp_next_page = circular.take().first().cloned();
                }
                let (mut output, col_style) = self.format_event(evt);
                let mut sublines: Vec<&str> = evt.msg.lines().rev().collect();
                output.push_str(sublines.pop().unwrap());
                for subline in sublines {
                    lines.push((col_style, indent, subline.to_string()));
                }
                lines.push((col_style, 0, output));
                if lines.len() == la_height {
                    break;
                }
                if opt_timestamp_prev_page.is_none() && lines.len() >= la_height / 2 {
                    opt_timestamp_prev_page = Some(evt.timestamp);
                }
            }
            state.opt_timestamp_prev_page = opt_timestamp_prev_page.or(state.opt_timestamp_bottom);
        }
        let la_left = list_area.left();
        let la_top = list_area.top();
        let la_width = list_area.width as usize;

        // lines is a vector with bottom line at index 0
        // wrapped_lines will be a vector with top line first
        let mut wrapped_lines = CircularBuffer::new(la_height);
        let rem_width = la_width - indent as usize;
        while let Some((style, left, line)) = lines.pop() {
            if line.chars().count() > la_width {
                wrapped_lines.push((style, left, line.chars().take(la_width).collect()));
                let mut remain: String = line.chars().skip(la_width).collect();
                while remain.chars().count() > rem_width {
                    let remove: String = remain.chars().take(rem_width).collect();
                    wrapped_lines.push((style, indent, remove));
                    remain = remain.chars().skip(rem_width).collect();
                }
                wrapped_lines.push((style, indent, remain.to_owned()));
            } else {
                wrapped_lines.push((style, left, line));
            }
        }

        let offset: u16 = if state.opt_timestamp_bottom.is_none() {
            0
        } else {
            let lines_cnt = wrapped_lines.len();
            (la_height - lines_cnt) as u16
        };

        for (i, (sty, left, l)) in wrapped_lines.iter().enumerate() {
            buf.set_stringn(
                la_left + left,
                la_top + i as u16 + offset,
                l,
                l.len(),
                sty.unwrap_or(self.style),
            );
        }
    }
}
