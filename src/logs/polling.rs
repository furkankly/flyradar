use std::time::Duration;

use backon::{ExponentialBuilder, Retryable};
use color_eyre::eyre;
use futures::stream::BoxStream;
use reqwest::StatusCode;
use tokio::task::{
    JoinHandle, {self},
};
use tracing::info;

use super::entry::LogEntry;
use super::{LogOptions, LogStream};
use crate::fly_rust::request_builder::RequestBuilderFly;
use crate::fly_rust::resource_logs::get_app_logs;
use crate::state::RdrResult;

#[derive(Debug)]
pub struct PollingStream {
    pub request_builder_fly: RequestBuilderFly,
}

impl LogStream for PollingStream {
    fn stream(
        &self,
        opts: &LogOptions,
    ) -> (BoxStream<'static, RdrResult<LogEntry>>, JoinHandle<()>) {
        let (tx, rx) = tokio::sync::mpsc::channel(100);
        let request_builder_fly_clone = self.request_builder_fly.clone();
        let opts_clone = opts.clone();
        let poll_handle = task::spawn(async move {
            info!("Polling stream task started");
            if let Err(e) = poll(&request_builder_fly_clone, &opts_clone, tx).await {
                // Log error if needed
                tracing::error!("Polling error: {}", e);
            }

            info!("Polling stream task ended");
        });

        (
            Box::pin(tokio_stream::wrappers::ReceiverStream::new(rx)),
            poll_handle,
        )
    }
}

const MIN_WAIT: Duration = Duration::from_millis(64);
const MAX_WAIT: Duration = Duration::from_millis(4096);

pub async fn poll(
    request_builder_fly: &RequestBuilderFly,
    opts: &LogOptions,
    tx: tokio::sync::mpsc::Sender<RdrResult<LogEntry>>,
) -> RdrResult<()> {
    let mut next_token = None;

    let retry_policy = ExponentialBuilder::default()
        .with_min_delay(MIN_WAIT)
        .with_max_delay(MAX_WAIT)
        .with_max_times(10);

    loop {
        info!("Polling logs...");
        let (new_token, was_empty) = (|| async {
            let (entries, token) = get_app_logs(
                request_builder_fly,
                &opts.app_name,
                next_token.clone(),
                opts.vm_id.clone(),
                opts.region_code.clone(),
            )
            .await?;
            if entries.is_empty() {
                Ok((token, true))
            } else {
                for entry in entries {
                    tx.send(Ok(entry)).await?;
                }
                Ok((token, false))
            }
        })
        .retry(retry_policy)
        .sleep(tokio::time::sleep)
        .when(|e: &eyre::Report| {
            e.downcast_ref::<reqwest::Error>().is_none_or(|req_err| {
                req_err.status().is_none_or(|status| {
                    !(status == StatusCode::NOT_FOUND || status == StatusCode::UNAUTHORIZED)
                })
            })
        })
        .await?;

        if !new_token.is_empty() {
            next_token = Some(new_token);
        }

        if opts.no_tail {
            return Ok(());
        }

        if was_empty {
            tokio::time::sleep(MIN_WAIT * 2).await;
        }
    }
}
