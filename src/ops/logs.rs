use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use futures::stream::select_all;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tokio::time::{sleep, Duration};
use tokio_stream::StreamExt;
use tokio_util::sync::CancellationToken;
use tracing::info;

use crate::logs::nats::NatsLogStream;
use crate::logs::polling::PollingStream;
use crate::logs::{LogOptions, LogStream};
use crate::ops::Ops;
use crate::state::RdrResult;
use crate::widgets::log_viewer::{cleanup_logger, init_logger, Drain, LevelFilter};

pub async fn dump_file_path(resource_info: String) -> RdrResult<PathBuf> {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let filename = format!("{}_{}.log", resource_info, timestamp);
    Ok(tokio::fs::canonicalize(".").await?.join(filename))
}

pub struct LogsResources {
    //INFO: This watcher is used to cancel and cleanup a possible ongoing establishment of nats connection. (by dropping the future that establishes the conn.)
    pub cancellation_token_nats: CancellationToken,
    pub polling_handle: Option<JoinHandle<()>>,
    pub nats: Option<NatsLogStream>,
}

pub async fn logs(
    ops: &mut Ops,
    opts: &LogOptions,
    cancellation_token_nats: CancellationToken,
) -> RdrResult<()> {
    let mut streams = Vec::new();

    if opts.no_tail {
        let polling_stream = PollingStream {
            request_builder_fly: ops.request_builder_fly.clone(),
        };
        let (stream, polling_handle) = polling_stream.stream(opts);
        streams.push(stream);
        polling_handle.abort();
    } else {
        let (_logs_tx, logs_rx) = mpsc::channel(100);
        // Start polling stream
        let polling_stream = PollingStream {
            request_builder_fly: ops.request_builder_fly.clone(),
        };
        let (polling_stream, polling_handle) = polling_stream.stream(opts);
        {
            let mut shared_state_guard = ops.shared_state.lock().unwrap();
            shared_state_guard.logs_rx = Some(logs_rx);
        }

        {
            let mut logs_resources_guard = ops.logs_resources.lock().unwrap();
            logs_resources_guard.polling_handle = Some(polling_handle);
        }

        streams.push(polling_stream);

        let nats_connect_fut = NatsLogStream::new(&ops.request_builder_graphql, opts);
        info!("before ");
        tokio::select! {
            // Try to connect to NATS
            nats_connect_result = nats_connect_fut => {
            match nats_connect_result {
                Ok(nats) => {
                    let logs_resources_clone = ops.logs_resources.clone();
                    // Successfully connected to NATS
                    tokio::spawn(async move {
                        // Wait 2 seconds before cancelling polling
                        sleep(Duration::from_secs(2)).await;
                        // Abort the polling task
                        if let Some(handle) = logs_resources_clone.lock().unwrap().polling_handle.take()
                        {
                            info!("aborting the polling task");
                            handle.abort();
                        }
                    });
                    let (stream, _handle) = nats.stream(opts);
                    streams.push(stream);
                    let mut logs_resources_guard = ops.logs_resources.lock().unwrap();
                    logs_resources_guard.nats = Some(nats);
                }
                Err(e) => {
                    info!("Could not connect to NATS: {}", e);
                    info!("Continuing with polling only...");
                }}
            }
            _ = cancellation_token_nats.cancelled() => {
                info!("cancelled ");
                return Ok(());
            }
        }

        let drain = Drain::new();
        init_logger(LevelFilter::Trace)?;
        // Combine all active streams
        let mut combined = select_all(streams);
        while let Some(entry) = combined.next().await {
            let record = entry?;
            drain.log(&record);
            // if logs_tx.send(entry).await.is_err() {
            //     info!("Receiver dropped, exiting stream loop");
            //     break;
            // }
        }
        cleanup_logger();
    }

    Ok(())
}
