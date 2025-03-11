mod circular_buffer;
mod inner;
mod smart;
mod standard;

use std::collections::hash_map::{Iter, Keys};
use std::collections::HashMap;
use std::io::Error;
use std::path::PathBuf;
use std::sync::{mpsc, Arc};
use std::{cmp, mem, thread};

use chrono::{DateTime, Utc};
use circular_buffer::CircularBuffer;
pub use inner::TuiWidgetState;
use inner::{TuiLoggerInner, TuiWidgetInnerState};
use lazy_static::lazy_static;
use parking_lot::Mutex;
// use log::{Log, Metadata, Record, SetLoggerError};
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Modifier, Style},
    widgets::{Block, Widget},
};
pub use smart::TuiLoggerSmartWidget;
pub use standard::TuiLoggerWidget;
use strip_ansi_escapes::strip;
use tracing::info;

use crate::logs::entry::{Error as SetLoggerError, Event, LogEntry as Record, Meta};

pub mod file;
pub use file::TuiLoggerFile;
#[repr(usize)]
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug, Hash)]
pub enum Level {
    Error = 1,
    Warn,
    Info,
    Debug,
    Trace,
}
impl Level {
    fn from_usize(u: usize) -> Option<Level> {
        match u {
            1 => Some(Level::Error),
            2 => Some(Level::Warn),
            3 => Some(Level::Info),
            4 => Some(Level::Debug),
            5 => Some(Level::Trace),
            _ => None,
        }
    }
    /// Converts the `Level` to the equivalent `LevelFilter`.
    #[inline]
    pub fn to_level_filter(&self) -> LevelFilter {
        LevelFilter::from_usize(*self as usize).unwrap()
    }
}

impl PartialEq<LevelFilter> for Level {
    #[inline]
    fn eq(&self, other: &LevelFilter) -> bool {
        *self as usize == *other as usize
    }
}

impl PartialOrd<LevelFilter> for Level {
    #[inline]
    fn partial_cmp(&self, other: &LevelFilter) -> Option<cmp::Ordering> {
        Some((*self as usize).cmp(&(*other as usize)))
    }
}
#[repr(usize)]
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug, Hash)]
pub enum LevelFilter {
    Off,
    Error,
    Warn,
    Info,
    Debug,
    Trace,
}
impl LevelFilter {
    fn from_usize(u: usize) -> Option<LevelFilter> {
        match u {
            0 => Some(LevelFilter::Off),
            1 => Some(LevelFilter::Error),
            2 => Some(LevelFilter::Warn),
            3 => Some(LevelFilter::Info),
            4 => Some(LevelFilter::Debug),
            5 => Some(LevelFilter::Trace),
            _ => None,
        }
    }

    /// Returns the most verbose logging level filter.
    #[inline]
    pub fn max() -> LevelFilter {
        LevelFilter::Trace
    }

    /// Converts `self` to the equivalent `Level`.
    ///
    /// Returns `None` if `self` is `LevelFilter::Off`.
    #[inline]
    pub fn to_level(&self) -> Option<Level> {
        Level::from_usize(*self as usize)
    }
}
impl PartialEq<Level> for LevelFilter {
    #[inline]
    fn eq(&self, other: &Level) -> bool {
        other.eq(self)
    }
}

impl PartialOrd<Level> for LevelFilter {
    #[inline]
    fn partial_cmp(&self, other: &Level) -> Option<cmp::Ordering> {
        Some((*self as usize).cmp(&(*other as usize)))
    }
}

#[derive(Clone)]
struct ExtLogRecord {
    timestamp: DateTime<Utc>,
    instance: String,
    level: Level,
    target: String,
    msg: String,
    meta: Meta,
}

fn advance_levelfilter(levelfilter: LevelFilter) -> (Option<LevelFilter>, Option<LevelFilter>) {
    match levelfilter {
        LevelFilter::Trace => (None, Some(LevelFilter::Debug)),
        LevelFilter::Debug => (Some(LevelFilter::Trace), Some(LevelFilter::Info)),
        LevelFilter::Info => (Some(LevelFilter::Debug), Some(LevelFilter::Warn)),
        LevelFilter::Warn => (Some(LevelFilter::Info), Some(LevelFilter::Error)),
        LevelFilter::Error => (Some(LevelFilter::Warn), Some(LevelFilter::Off)),
        LevelFilter::Off => (Some(LevelFilter::Error), None),
    }
}

/// LevelConfig stores the relation target->LevelFilter in a hash table.
///
/// The table supports copying from the logger system LevelConfig to
/// a widget's LevelConfig. In order to detect changes, the generation
/// of the hash table is compared with any previous copied table.
/// On every change the generation is incremented.
#[derive(Default)]
pub struct LevelConfig {
    config: HashMap<String, LevelFilter>,
    generation: u64,
    origin_generation: u64,
    default_display_level: Option<LevelFilter>,
}
impl LevelConfig {
    /// Create an empty LevelConfig.
    pub fn new() -> LevelConfig {
        LevelConfig {
            config: HashMap::new(),
            generation: 0,
            origin_generation: 0,
            default_display_level: None,
        }
    }
    /// Set for a given target the LevelFilter in the table and update the generation.
    pub fn set(&mut self, target: &str, level: LevelFilter) {
        if let Some(lev) = self.config.get_mut(target) {
            if *lev != level {
                *lev = level;
                self.generation += 1;
            }
            return;
        }
        self.config.insert(target.to_string(), level);
        self.generation += 1;
    }
    /// Set default display level filter for new targets - independent from recording
    pub fn set_default_display_level(&mut self, level: LevelFilter) {
        self.default_display_level = Some(level);
    }
    /// Retrieve an iter for all the targets stored in the hash table.
    pub fn keys(&self) -> Keys<String, LevelFilter> {
        self.config.keys()
    }
    /// Get the levelfilter for a given target.
    pub fn get(&self, target: &str) -> Option<LevelFilter> {
        self.config.get(target).cloned()
    }
    /// Retrieve an iterator through all entries of the table.
    pub fn iter(&self) -> Iter<String, LevelFilter> {
        self.config.iter()
    }
    /// Merge an origin LevelConfig into this one.
    ///
    /// The origin table defines the maximum levelfilter.
    /// If this table has a higher levelfilter, then it will be reduced.
    /// Unknown targets will be copied to this table.
    fn merge(&mut self, origin: &LevelConfig) {
        if self.origin_generation != origin.generation {
            for (target, origin_levelfilter) in origin.iter() {
                if let Some(levelfilter) = self.get(target) {
                    if levelfilter <= *origin_levelfilter {
                        continue;
                    }
                }
                let levelfilter = self
                    .default_display_level
                    .map(|lvl| {
                        if lvl > *origin_levelfilter {
                            *origin_levelfilter
                        } else {
                            lvl
                        }
                    })
                    .unwrap_or(*origin_levelfilter);
                self.set(target, levelfilter);
            }
            self.generation = origin.generation;
        }
    }
}

/// These are the sub-structs for the static TUI_LOGGER struct.
struct HotSelect {
    hashtable: HashMap<u64, LevelFilter>,
    default: LevelFilter,
}
struct HotLog {
    events: CircularBuffer<ExtLogRecord>,
    mover_thread: Option<thread::JoinHandle<()>>,
    shutdown_mover_tx: Option<mpsc::Sender<()>>,
}

struct TuiLogger {
    hot_select: Mutex<HotSelect>,
    hot_log: Mutex<HotLog>,
    inner: Mutex<TuiLoggerInner>,
}
impl TuiLogger {
    pub fn move_events(&self) {
        // If there are no new events, then just return
        if self.hot_log.lock().events.total_elements() == 0 {
            info!("From move events thread: No events");
            return;
        }
        // Exchange new event buffer with the hot buffer
        let mut received_events = {
            let hot_depth = self.inner.lock().hot_depth;
            let new_circular = CircularBuffer::new(hot_depth);
            let mut hl = self.hot_log.lock();
            mem::replace(&mut hl.events, new_circular)
        };
        let mut tli = self.inner.lock();
        let total = received_events.total_elements();
        let elements = received_events.len();
        tli.total_events += total;
        let mut consumed = received_events.take();
        let mut reversed = Vec::with_capacity(consumed.len() + 1);
        while let Some(log_entry) = consumed.pop() {
            reversed.push(log_entry);
        }
        if total > elements {
            // Too many events received, so some have been lost
            let new_log_entry = ExtLogRecord {
                timestamp: reversed[reversed.len() - 1].timestamp,
                instance: "".to_string(),
                level: Level::Warn,
                target: "TuiLogger".to_string(),
                msg: format!(
                    "There have been {} events lost, {} recorded out of {}",
                    total - elements,
                    elements,
                    total
                ),
                meta: Meta {
                    instance: "".to_string(),
                    region: "".to_string(),
                    event: Event {
                        provider: "".to_string(),
                    },
                    http: None,
                    error: None,
                    url: None,
                },
            };
            reversed.push(new_log_entry);
        }
        let default_level = tli.default;
        while let Some(log_entry) = reversed.pop() {
            if tli.targets.get(&log_entry.target).is_none() {
                tli.targets.set(&log_entry.target, default_level);
            }
            // if let Some(ref mut file_options) = tli.dump {
            //     let mut output = String::new();
            //     let (lev_long, lev_abbr, with_loc) = match log_entry.level {
            //         Level::Error => ("ERROR", "E", true),
            //         Level::Warn => ("WARN ", "W", true),
            //         Level::Info => ("INFO ", "I", false),
            //         Level::Debug => ("DEBUG", "D", true),
            //         Level::Trace => ("TRACE", "T", true),
            //     };
            //     if let Some(fmt) = file_options.timestamp_fmt.as_ref() {
            //         output.push_str(&format!("{}", log_entry.timestamp.format(fmt)));
            //         output.push(file_options.format_separator);
            //     }
            //     match file_options.format_output_level {
            //         None => {}
            //         Some(TuiLoggerLevelOutput::Abbreviated) => {
            //             output.push_str(lev_abbr);
            //             output.push(file_options.format_separator);
            //         }
            //         Some(TuiLoggerLevelOutput::Long) => {
            //             output.push_str(lev_long);
            //             output.push(file_options.format_separator);
            //         }
            //     }
            //     if file_options.format_output_target {
            //         output.push_str(&log_entry.target);
            //         output.push(file_options.format_separator);
            //     }
            //     if with_loc {
            //         if file_options.format_output_file {
            //             output.push_str(&log_entry.file);
            //             output.push(file_options.format_separator);
            //         }
            //         if file_options.format_output_line {
            //             output.push_str(&format!("{}", log_entry.line));
            //             output.push(file_options.format_separator);
            //         }
            //     }
            //     output.push_str(&log_entry.msg);
            //     if let Err(_e) = writeln!(file_options.dump, "{}", output) {
            //         // TODO: What to do in case of write error ?
            //     }
            // }
            tli.events.push(log_entry);
        }
    }
}
lazy_static! {
    static ref TUI_LOGGER: TuiLogger = {
        let hs = HotSelect {
            hashtable: HashMap::with_capacity(1000),
            default: LevelFilter::Info,
        };
        let hl = HotLog {
            events: CircularBuffer::new(1000),
            mover_thread: None,
            shutdown_mover_tx: None,
        };
        let tli = TuiLoggerInner {
            hot_depth: 1000,
            events: CircularBuffer::new(10000),
            total_events: 0,
            dump: None,
            default: LevelFilter::Info,
            targets: LevelConfig::new(),
        };
        TuiLogger {
            hot_select: Mutex::new(hs),
            hot_log: Mutex::new(hl),
            inner: Mutex::new(tli),
        }
    };
}

// Lots of boilerplate code, so that init_logger can return two error types...
#[derive(Debug)]
pub enum TuiLoggerError {
    SetLoggerError(SetLoggerError),
    ThreadError(std::io::Error),
}
impl std::error::Error for TuiLoggerError {
    fn description(&self) -> &str {
        match self {
            TuiLoggerError::SetLoggerError(_) => "SetLoggerError",
            TuiLoggerError::ThreadError(_) => "ThreadError",
        }
    }
    fn cause(&self) -> Option<&dyn std::error::Error> {
        match self {
            TuiLoggerError::SetLoggerError(_) => None,
            TuiLoggerError::ThreadError(err) => Some(err),
        }
    }
}
impl std::fmt::Display for TuiLoggerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TuiLoggerError::SetLoggerError(err) => write!(f, "SetLoggerError({})", err),
            TuiLoggerError::ThreadError(err) => write!(f, "ThreadError({})", err),
        }
    }
}

/// Init the logger.
pub fn init_logger(max_level: LevelFilter) -> Result<(), TuiLoggerError> {
    let mut hot_log = TUI_LOGGER.hot_log.lock();
    if hot_log.mover_thread.is_some() {
        return Ok(());
    }
    let (shutdown_mover_tx, shutdown_mover_rx) = mpsc::channel();
    let join_handle = thread::Builder::new()
        .name("tui-logger::move_events".into())
        .spawn(move || {
            let duration = std::time::Duration::from_millis(500);
            loop {
                if shutdown_mover_rx.recv_timeout(duration).is_ok() {
                    break;
                }
                thread::park_timeout(duration);
                TUI_LOGGER.move_events();
            }
        })
        .map_err(TuiLoggerError::ThreadError)?;
    hot_log.mover_thread = Some(join_handle);
    hot_log.shutdown_mover_tx = Some(shutdown_mover_tx);
    set_default_level(max_level);
    Ok(())
}

pub fn cleanup_logger() {
    info!("Cleaning up the tui-logger.");
    // {
    //     let mut hl = TUI_LOGGER.hot_log.lock();
    //     if let Some(tx) = hl.shutdown_mover_tx.take() {
    //         let thread = hl.mover_thread.take();
    //         let _ = tx.send(());
    //         if let Some(thread) = thread {
    //             let _ = thread.join();
    //         }
    //     }
    // }
    // Reset all the buffers and state
    let mut tli = TUI_LOGGER.inner.lock();
    let mut hl = TUI_LOGGER.hot_log.lock();
    let mut hs = TUI_LOGGER.hot_select.lock();
    tli.events.clear();
    tli.total_events = 0;
    tli.targets = LevelConfig::new();
    hl.events.clear();
    hs.hashtable.clear();
}

// INFO: making this part of inner state and dumping whats on the screen (display filter+focus) would require me to make this part of shared state at app side (behind arcmutex) which would lead to holding the lock on every transition that disregards the whole point of double buffering. but I prob. don't need double buffering for flyradar.
// TODO: I prob don't need the double buffering of tui-logger for this app.
// INFO: the only async func here cuz i dont want to block tokio's thread pool while dumping.
pub async fn dump_logs(file_path: &PathBuf) -> Result<(), Error> {
    let file = tokio::fs::OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .open(&file_path)
        .await?;
    let mut buf_writer = tokio::io::BufWriter::new(file);

    let formatted_logs = {
        let tui_lock = TUI_LOGGER.inner.lock();
        let file_options = if let Some(file_opts) = tui_lock.dump.as_ref() {
            TuiLoggerFile {
                format_separator: file_opts.format_separator,
                timestamp_fmt: file_opts.timestamp_fmt.clone(),
                format_output_file: file_opts.format_output_file,
                format_output_line: file_opts.format_output_line,
                format_output_target: file_opts.format_output_target,
                format_output_level: file_opts.format_output_level,
            }
        } else {
            TuiLoggerFile::new()
        };

        tui_lock
            .events
            .rev_iter()
            .filter_map(|evt| {
                let should_dump = match tui_lock.targets.config.get(&evt.target) {
                    Some(level) => level >= &evt.level,
                    None => tui_lock
                        .targets
                        .default_display_level
                        .is_some_and(|default_level| default_level >= evt.level),
                };

                should_dump.then(|| {
                    let mut output = String::new();
                    let (lev_long, lev_abbr) = match evt.level {
                        Level::Error => ("ERROR", "E"),
                        Level::Warn => ("WARN", "W"),
                        Level::Info => ("INFO", "I"),
                        Level::Debug => ("DEBUG", "D"),
                        Level::Trace => ("TRACE", "T"),
                    };

                    if let Some(fmt) = &file_options.timestamp_fmt {
                        output.push_str(&format!("{}", evt.timestamp.format(fmt)));
                        output.push(file_options.format_separator);
                    }

                    match file_options.format_output_level {
                        None => {}
                        Some(TuiLoggerLevelOutput::Abbreviated) => {
                            output.push_str(lev_abbr);
                            output.push(file_options.format_separator);
                        }
                        Some(TuiLoggerLevelOutput::Long) => {
                            output.push_str(lev_long);
                            output.push(file_options.format_separator);
                        }
                    }

                    if file_options.format_output_target {
                        output.push_str(&evt.target);
                        output.push(file_options.format_separator);
                    }

                    output.push_str(&evt.msg);
                    output.push('\n');
                    output
                })
            })
            .collect::<Vec<_>>()
    };

    for log in formatted_logs {
        tokio::io::AsyncWriteExt::write_all(&mut buf_writer, log.as_bytes()).await?;
    }

    tokio::io::AsyncWriteExt::flush(&mut buf_writer).await?;
    Ok(())
}

/// Set the depth of the hot buffer in order to avoid message loss.
/// This is effective only after a call to move_events()
pub fn set_hot_buffer_depth(depth: usize) {
    TUI_LOGGER.inner.lock().hot_depth = depth;
}

/// Set the depth of the circular buffer in order to avoid message loss.
/// This will delete all existing messages in the circular buffer.
pub fn set_buffer_depth(depth: usize) {
    TUI_LOGGER.inner.lock().events = CircularBuffer::new(depth);
}

// Define filename and log formmating options for file dumping.
// pub fn set_log_file(file_options: TuiLoggerFile) {
//     TUI_LOGGER.inner.lock().dump = Some(file_options);
// }

/// Set default levelfilter for unknown targets of the logger
pub fn set_default_level(levelfilter: LevelFilter) {
    TUI_LOGGER.hot_select.lock().default = levelfilter;
    TUI_LOGGER.inner.lock().default = levelfilter;
}

/// Set levelfilter for a specific target in the logger
pub fn set_level_for_target(target: &str, levelfilter: LevelFilter) {
    let h = fxhash::hash64(&target);
    TUI_LOGGER.inner.lock().targets.set(target, levelfilter);
    let mut hs = TUI_LOGGER.hot_select.lock();
    hs.hashtable.insert(h, levelfilter);
}

impl TuiLogger {
    fn raw_log(&self, record: &Record) {
        let log_entry = ExtLogRecord {
            timestamp: DateTime::parse_from_rfc3339(&record.timestamp)
                .unwrap()
                .with_timezone(&Utc),
            instance: record.instance.clone(),
            level: record.map_level(),
            target: record.region.clone(),
            msg: String::from_utf8_lossy(&strip(record.message.as_bytes())).to_string(),
            meta: Meta {
                instance: record.meta.instance.clone(),
                region: record.region.clone(),
                event: Event {
                    provider: record.meta.event.provider.clone(),
                },
                http: record.meta.http.clone(),
                error: record.meta.error.clone(),
                url: record.meta.url.clone(),
            },
        };
        let mut events_lock = self.hot_log.lock();
        events_lock.events.push(log_entry);
        let need_signal =
            (events_lock.events.total_elements() % (events_lock.events.capacity() / 2)) == 0;
        if need_signal {
            if let Some(jh) = events_lock.mover_thread.as_ref() {
                thread::Thread::unpark(jh.thread())
            }
        }
    }
}

/// A simple `Drain` to log any event directly.
#[derive(Default)]
pub struct Drain;

impl Drain {
    /// Create a new Drain
    pub fn new() -> Self {
        Drain
    }
    fn enabled(&self, record: &Record) -> bool {
        let h = fxhash::hash64(&record.region);
        let hs = TUI_LOGGER.hot_select.lock();
        if let Some(&levelfilter) = hs.hashtable.get(&h) {
            record.map_level() <= levelfilter
        } else {
            record.map_level() <= hs.default
        }
    }
    /// Log the given record to the main tui-logger
    pub fn log(&self, record: &Record) {
        info!("RECORD: {:#?}", record);
        if self.enabled(record) {
            TUI_LOGGER.raw_log(record)
        }
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub enum TuiWidgetEvent {
    SpaceKey,
    UpKey,
    DownKey,
    LeftKey,
    RightKey,
    PlusKey,
    MinusKey,
    HideKey,
    FocusKey,
    PrevPageKey,
    NextPageKey,
    EscapeKey,
}

/// This is the definition for the TuiLoggerTargetWidget,
/// which allows configuration of the logger system and selection of log messages.
pub struct TuiLoggerTargetWidget<'b> {
    block: Option<Block<'b>>,
    /// Base style of the widget
    style: Style,
    style_show: Style,
    style_hide: Style,
    style_off: Option<Style>,
    highlight_style: Style,
    state: Arc<Mutex<TuiWidgetInnerState>>,
    targets: Vec<String>,
}
impl<'b> Default for TuiLoggerTargetWidget<'b> {
    fn default() -> TuiLoggerTargetWidget<'b> {
        //TUI_LOGGER.move_events();
        TuiLoggerTargetWidget {
            block: None,
            style: Default::default(),
            style_off: None,
            style_hide: Style::default(),
            style_show: Style::default().add_modifier(Modifier::REVERSED),
            highlight_style: Style::default().add_modifier(Modifier::REVERSED),
            state: Arc::new(Mutex::new(TuiWidgetInnerState::new())),
            targets: vec![],
        }
    }
}
impl<'b> TuiLoggerTargetWidget<'b> {
    pub fn block(mut self, block: Block<'b>) -> TuiLoggerTargetWidget<'b> {
        self.block = Some(block);
        self
    }
    fn opt_style(mut self, style: Option<Style>) -> TuiLoggerTargetWidget<'b> {
        if let Some(s) = style {
            self.style = s;
        }
        self
    }
    fn opt_style_off(mut self, style: Option<Style>) -> TuiLoggerTargetWidget<'b> {
        if style.is_some() {
            self.style_off = style;
        }
        self
    }
    fn opt_style_hide(mut self, style: Option<Style>) -> TuiLoggerTargetWidget<'b> {
        if let Some(s) = style {
            self.style_hide = s;
        }
        self
    }
    fn opt_style_show(mut self, style: Option<Style>) -> TuiLoggerTargetWidget<'b> {
        if let Some(s) = style {
            self.style_show = s;
        }
        self
    }
    fn opt_highlight_style(mut self, style: Option<Style>) -> TuiLoggerTargetWidget<'b> {
        if let Some(s) = style {
            self.highlight_style = s;
        }
        self
    }
    pub fn style(mut self, style: Style) -> TuiLoggerTargetWidget<'b> {
        self.style = style;
        self
    }
    pub fn style_off(mut self, style: Style) -> TuiLoggerTargetWidget<'b> {
        self.style_off = Some(style);
        self
    }
    pub fn style_hide(mut self, style: Style) -> TuiLoggerTargetWidget<'b> {
        self.style_hide = style;
        self
    }
    pub fn style_show(mut self, style: Style) -> TuiLoggerTargetWidget<'b> {
        self.style_show = style;
        self
    }
    pub fn highlight_style(mut self, style: Style) -> TuiLoggerTargetWidget<'b> {
        self.highlight_style = style;
        self
    }
    fn inner_state(mut self, state: Arc<Mutex<TuiWidgetInnerState>>) -> TuiLoggerTargetWidget<'b> {
        self.state = state;
        self
    }
    pub fn state(mut self, state: &TuiWidgetState) -> TuiLoggerTargetWidget<'b> {
        self.state = state.inner.clone();
        self
    }
}
impl Widget for TuiLoggerTargetWidget<'_> {
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
        if list_area.width < 8 || list_area.height < 1 {
            return;
        }

        let la_left = list_area.left();
        let la_top = list_area.top();
        let la_width = list_area.width as usize;

        {
            let inner = &TUI_LOGGER.inner.lock();
            let hot_targets = &inner.targets;
            let mut state = self.state.lock();
            let hide_off = state.hide_off;
            let offset = state.offset;
            let focus_selected = state.focus_selected;
            {
                let targets = &mut state.config;
                targets.merge(hot_targets);
                self.targets.clear();
                for (t, levelfilter) in targets.iter() {
                    if hide_off && levelfilter == &LevelFilter::Off {
                        continue;
                    }
                    self.targets.push(t.clone());
                }
                self.targets.sort();
            }
            state.nr_items = self.targets.len();
            if state.selected >= state.nr_items {
                state.selected = state.nr_items.max(1) - 1;
            }
            if state.selected < state.nr_items {
                state.opt_selected_target = Some(self.targets[state.selected].clone());
                let t = &self.targets[state.selected];
                let (more, less) = if let Some(levelfilter) = state.config.get(t) {
                    advance_levelfilter(levelfilter)
                } else {
                    (None, None)
                };
                state.opt_selected_visibility_less = less;
                state.opt_selected_visibility_more = more;
                let (more, less) = if let Some(levelfilter) = hot_targets.get(t) {
                    advance_levelfilter(levelfilter)
                } else {
                    (None, None)
                };
                state.opt_selected_recording_less = less;
                state.opt_selected_recording_more = more;
            }
            let list_height = (list_area.height as usize).min(self.targets.len());
            let offset = if list_height > self.targets.len() {
                0
            } else if state.selected < state.nr_items {
                let sel = state.selected;
                if sel >= offset + list_height {
                    // selected is below visible list range => make it the bottom
                    sel - list_height + 1
                } else if sel.min(offset) + list_height > self.targets.len() {
                    self.targets.len() - list_height
                } else {
                    sel.min(offset)
                }
            } else {
                0
            };
            state.offset = offset;

            let targets = &(&state.config);
            let default_level = inner.default;
            for i in 0..list_height {
                let t = &self.targets[i + offset];
                // Comment in relation to issue #69:
                // Widgets maintain their own list of level filters per target.
                // These lists are not forwarded to the TUI_LOGGER, but kept widget private.
                // Example: This widget's private list contains a target named "not_yet",
                // and the application hasn't logged an entry with target "not_yet".
                // If displaying the target list, then "not_yet" will be only present in target,
                // but not in hot_targets. In issue #69 the problem has been, that
                // `hot_targets.get(t).unwrap()` has caused a panic. Which is to be expected.
                // The remedy is to use unwrap_or with default_level.
                let hot_level_filter = hot_targets.get(t).unwrap_or(default_level);
                let level_filter = targets.get(t).unwrap_or(default_level);
                for (j, sym, lev) in &[
                    (0, "E", Level::Error),
                    (1, "W", Level::Warn),
                    (2, "I", Level::Info),
                    (3, "D", Level::Debug),
                    (4, "T", Level::Trace),
                ] {
                    if let Some(cell) = buf.cell_mut((la_left + j, la_top + i as u16)) {
                        let cell_style = if hot_level_filter >= *lev {
                            if level_filter >= *lev {
                                if !focus_selected || i + offset == state.selected {
                                    self.style_show
                                } else {
                                    self.style_hide
                                }
                            } else {
                                self.style_hide
                            }
                        } else if let Some(style_off) = self.style_off {
                            style_off
                        } else {
                            cell.set_symbol(" ");
                            continue;
                        };
                        cell.set_style(cell_style);
                        cell.set_symbol(sym);
                    }
                }
                buf.set_stringn(la_left + 5, la_top + i as u16, ":", la_width, self.style);
                buf.set_stringn(
                    la_left + 6,
                    la_top + i as u16,
                    t,
                    la_width,
                    if i + offset == state.selected {
                        self.highlight_style
                    } else {
                        self.style
                    },
                );
            }
        }
    }
}

/// The TuiLoggerWidget shows the logging messages in an endless scrolling view.
/// It is controlled by a TuiWidgetState for selected events.
#[derive(Debug, Clone, Copy, PartialEq, Hash)]
pub enum TuiLoggerLevelOutput {
    Abbreviated,
    Long,
}
