use std::sync::Arc;

use chrono::{DateTime, Utc};
use parking_lot::Mutex;

use crate::widgets::log_viewer::file::TuiLoggerFile;
use crate::widgets::log_viewer::{
    set_level_for_target, CircularBuffer, ExtLogRecord, LevelConfig, LevelFilter, TuiWidgetEvent,
};

pub struct TuiLoggerInner {
    pub hot_depth: usize,
    pub events: CircularBuffer<ExtLogRecord>,
    pub dump: Option<TuiLoggerFile>,
    pub total_events: usize,
    pub default: LevelFilter,
    pub targets: LevelConfig,
}

/// This struct contains the shared state of a TuiLoggerWidget and a TuiLoggerTargetWidget.
#[derive(Default)]
pub struct TuiWidgetState {
    pub inner: Arc<Mutex<TuiWidgetInnerState>>,
}
impl TuiWidgetState {
    /// Create a new TuiWidgetState
    pub fn new() -> TuiWidgetState {
        TuiWidgetState {
            inner: Arc::new(Mutex::new(TuiWidgetInnerState::new())),
        }
    }
    pub fn set_default_display_level(self, levelfilter: LevelFilter) -> TuiWidgetState {
        self.inner.lock().config.default_display_level = Some(levelfilter);
        self
    }
    pub fn set_level_for_target(self, target: &str, levelfilter: LevelFilter) -> TuiWidgetState {
        self.inner.lock().config.set(target, levelfilter);
        self
    }
    pub fn transition(&mut self, event: TuiWidgetEvent) {
        self.inner.lock().transition(event);
    }
}

#[derive(Default)]
pub struct TuiWidgetInnerState {
    pub config: LevelConfig,
    pub nr_items: usize,
    pub selected: usize,
    pub opt_timestamp_bottom: Option<DateTime<Utc>>,
    pub opt_timestamp_next_page: Option<DateTime<Utc>>,
    pub opt_timestamp_prev_page: Option<DateTime<Utc>>,
    pub opt_selected_target: Option<String>,
    pub opt_selected_visibility_more: Option<LevelFilter>,
    pub opt_selected_visibility_less: Option<LevelFilter>,
    pub opt_selected_recording_more: Option<LevelFilter>,
    pub opt_selected_recording_less: Option<LevelFilter>,
    pub offset: usize,
    pub hide_off: bool,
    pub hide_target: bool,
    pub focus_selected: bool,
}
impl TuiWidgetInnerState {
    pub fn new() -> TuiWidgetInnerState {
        TuiWidgetInnerState::default()
    }
    fn transition(&mut self, event: TuiWidgetEvent) {
        use TuiWidgetEvent::*;
        match event {
            SpaceKey => {
                self.hide_off ^= true;
            }
            HideKey => {
                self.hide_target ^= true;
            }
            FocusKey => {
                self.focus_selected ^= true;
            }
            UpKey => {
                if !self.hide_target && self.selected > 0 {
                    self.selected -= 1;
                }
            }
            DownKey => {
                if !self.hide_target && self.selected + 1 < self.nr_items {
                    self.selected += 1;
                }
            }
            LeftKey => {
                if let Some(selected_target) = self.opt_selected_target.take() {
                    if let Some(selected_visibility_less) = self.opt_selected_visibility_less.take()
                    {
                        self.config.set(&selected_target, selected_visibility_less);
                    }
                }
            }
            RightKey => {
                if let Some(selected_target) = self.opt_selected_target.take() {
                    if let Some(selected_visibility_more) = self.opt_selected_visibility_more.take()
                    {
                        self.config.set(&selected_target, selected_visibility_more);
                    }
                }
            }
            PlusKey => {
                if let Some(selected_target) = self.opt_selected_target.take() {
                    if let Some(selected_recording_more) = self.opt_selected_recording_more.take() {
                        set_level_for_target(&selected_target, selected_recording_more);
                    }
                }
            }
            MinusKey => {
                if let Some(selected_target) = self.opt_selected_target.take() {
                    if let Some(selected_recording_less) = self.opt_selected_recording_less.take() {
                        set_level_for_target(&selected_target, selected_recording_less);
                    }
                }
            }
            PrevPageKey => self.opt_timestamp_bottom = self.opt_timestamp_prev_page,
            NextPageKey => self.opt_timestamp_bottom = self.opt_timestamp_next_page,
            EscapeKey => self.opt_timestamp_bottom = None,
        }
    }
}
