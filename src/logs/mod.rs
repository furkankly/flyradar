use entry::LogEntry;
use futures::stream::BoxStream;
use tokio::task::JoinHandle;

use crate::state::RdrResult;

pub mod entry;
pub mod nats;
pub mod polling;

pub trait LogStream {
    fn stream(
        &self,
        opts: &LogOptions,
    ) -> (BoxStream<'static, RdrResult<LogEntry>>, JoinHandle<()>);
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct LogOptions {
    pub app_name: String,
    pub vm_id: Option<String>,
    pub region_code: Option<String>,
    pub no_tail: bool,
}

impl LogOptions {
    pub fn to_nats_subject(&self) -> String {
        let mut parts = vec!["logs", &self.app_name as &str];
        parts.extend(
            [self.region_code.as_deref(), self.vm_id.as_deref()]
                .map(|opt| opt.unwrap_or("*"))
                .into_iter()
                .filter(|&s| !s.is_empty()),
        );
        parts.join(".")
    }
}
