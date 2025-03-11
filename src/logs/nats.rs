use std::net::IpAddr;
use std::sync::Arc;
use std::time::Duration;

use async_nats;
use color_eyre::eyre;
use futures::stream::BoxStream;
use futures::StreamExt;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tracing::info;

use super::entry::{Event, LogEntry, Meta, NatsLog};
use super::{LogOptions, LogStream};
use crate::agent::client::ClientDialer;
use crate::agent::{self};
use crate::auth::read_access_token;
use crate::fly_rust::request_builder::RequestBuilderGraphql;
use crate::fly_rust::resource_apps::get_app_basic;
use crate::state::RdrResult;

#[derive(Clone, Debug)]
pub struct NatsLogStream {
    pub nc: async_nats::Client,
}

impl NatsLogStream {
    pub async fn new(
        request_builder_graphql: &RequestBuilderGraphql,
        opts: &LogOptions,
    ) -> RdrResult<Self> {
        let app_basic = get_app_basic(request_builder_graphql, opts.app_name.clone())
            .await?
            .ok_or_else(|| eyre::eyre!("App not found: {}", opts.app_name.to_string()))?;

        let mut agent_client =
            agent::client::establish(request_builder_graphql, opts.app_name.clone()).await?;
        let org_slug = &app_basic.appbasic.organization.slug;

        let dialer = agent_client.dialer(org_slug, "").await?;
        agent_client.wait_for_tunnel(org_slug, "").await?;

        let nc = Self::new_nats_client(dialer, &app_basic.appbasic.organization.raw_slug).await?;

        Ok(Self { nc })
    }

    async fn new_nats_client(
        dialer: ClientDialer,
        org_slug: &str,
    ) -> RdrResult<async_nats::Client> {
        let state = dialer.state.clone();
        let peer_ip = state.peer.peer_ip.parse::<IpAddr>()?;

        let mut nats_ip_bytes = [0u8; 16];
        match peer_ip {
            IpAddr::V4(ipv4) => {
                nats_ip_bytes[..4].copy_from_slice(&ipv4.octets());
            }
            IpAddr::V6(ipv6) => {
                nats_ip_bytes[..6].copy_from_slice(&ipv6.octets()[..6]);
            }
        }
        nats_ip_bytes[15] = 3;
        let nats_ip = IpAddr::from(nats_ip_bytes);

        info!("dialer: {:#?}", dialer);

        let url = format!("ipc://[{}]:4223", nats_ip);
        let token = read_access_token().await?;
        let options = async_nats::ConnectOptions::new()
            .require_tls(false)
            .with_dialer(Arc::new(dialer.clone()))
            .user_and_password(org_slug.to_string(), token)
            .event_callback(|event| {
                tracing::info!("NATS Event: {:?}", event);
                Box::pin(async move {})
            })
            .ping_interval(Duration::from_secs(120));

        let client = options.connect(&url).await?;

        Ok(client)
    }
}

impl LogStream for NatsLogStream {
    fn stream(
        &self,
        opts: &LogOptions,
    ) -> (BoxStream<'static, RdrResult<LogEntry>>, JoinHandle<()>) {
        let (tx, rx) = mpsc::channel(100);

        let nc = self.nc.clone();
        let opts = opts.clone();

        let nats_handle = tokio::spawn(async move {
            tracing::info!("Starting NATS stream task");

            match from_nats(&nc, &opts, tx).await {
                Ok(()) => tracing::info!("NATS stream completed normally"),
                Err(e) => tracing::error!("NATS stream error: {}", e),
            }
        });

        (
            Box::pin(tokio_stream::wrappers::ReceiverStream::new(rx)),
            nats_handle,
        )
    }
}

async fn from_nats(
    nc: &async_nats::Client,
    opts: &LogOptions,
    tx: mpsc::Sender<RdrResult<LogEntry>>,
) -> RdrResult<()> {
    let subject = opts.to_nats_subject();
    tracing::info!("About to subscribe to: {}", subject);

    let mut sub = nc.subscribe(subject).await?;
    tracing::info!("Successfully subscribed to subject.");

    while let Some(msg) = sub.next().await {
        tracing::info!("Received NATS message");
        let log: NatsLog = serde_json::from_slice(&msg.payload)?;

        tx.send(Ok(LogEntry {
            instance: log.fly.app.instance.clone(),
            level: log.log.level,
            message: log.message,
            region: log.fly.region.clone(),
            timestamp: log.timestamp,
            meta: Meta {
                instance: log.fly.app.instance,
                region: log.fly.region,
                event: Event {
                    provider: log.event.provider,
                },
                http: None,
                error: None,
                url: None,
            },
        }))
        .await?;
    }

    Ok(())
}
