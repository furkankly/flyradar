use std::future::Future;
use std::net::IpAddr;
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use std::{io, time};

use async_nats_flyradar::connection::ipc_platform::connect_ipc;
use async_nats_flyradar::connection::{IpcStreamWrapper, NativeIpcStream};
use color_eyre::eyre::{
    Context, {self},
};
use futures::future::BoxFuture;
use graphql_client::{GraphQLQuery, Response};
use serde::{Deserialize, Serialize};
use tokio::process::Command;
use tokio::sync::Mutex;
use tracing::{error, info, instrument, warn};

use super::path_to_socket;
use crate::agent::errors::AgentError;
use crate::agent::proto;
use crate::agent::start::start_daemon;
use crate::auth::read_access_token;
use crate::fly_rust::request_builder::RequestBuilderGraphql;
use crate::state::RdrResult;
use crate::wireguard::{
    WireGuardState, {self},
};

const OK_PREFIX: &[u8] = b"ok ";
const ERROR_PREFIX: &[u8] = b"err ";
pub const AGENT_NOT_RUNNING: &str = "agent not running";

#[derive(Clone, Debug, Deserialize)]
pub struct ConfigPeer {
    pub public_key: String,
    pub endpoint: String,
    pub allowed_ips: Vec<String>,
    pub persistent_keepalive: u16,
}

#[derive(Debug, Deserialize)]
pub struct PingResponse {
    #[serde(rename = "PID")]
    _pid: i32,
    #[serde(rename = "Version")]
    version: String,
    #[serde(rename = "Background")]
    background: bool,
}

#[derive(Debug, Deserialize)]
pub struct EstablishResponse {
    #[serde(rename = "WireGuardState")]
    pub wireguard_state: Option<WireGuardState>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Instances {
    pub labels: Vec<String>,
    pub addresses: Vec<String>,
}

const CYCLE: Duration = Duration::from_millis(50);

#[derive(Clone, Debug)]
#[allow(dead_code)]
pub struct Client {
    network: String,
    address: PathBuf,
    token: Option<String>,
    agent_refused_tokens: bool,
}

fn is_prefixed_with(data: &[u8], prefix: &[u8]) -> bool {
    data.starts_with(prefix)
}

fn is_ok(data: &[u8]) -> bool {
    is_prefixed_with(data, OK_PREFIX)
}

fn extract_ok(data: &[u8]) -> &[u8] {
    &data[OK_PREFIX.len()..]
}

fn is_error(data: &[u8]) -> bool {
    is_prefixed_with(data, ERROR_PREFIX)
}

fn extract_error(data: &[u8]) -> eyre::Error {
    let msg = &data[ERROR_PREFIX.len()..];
    eyre::eyre!(String::from_utf8_lossy(msg).into_owned())
}

//INFO: flyradar-specific version checking for flyctl
async fn get_current_version() -> RdrResult<String> {
    let output = Command::new("fly")
        .arg("version")
        .output()
        .await
        .wrap_err("failed to execute fly version command")?;

    if !output.status.success() {
        return Err(eyre::eyre!("fly version command failed"));
    }

    let version_output =
        String::from_utf8(output.stdout).wrap_err("invalid utf8 in version output")?;

    // Extract version from output like "fly v0.3.53 darwin/arm64..."
    let version = version_output
        .split_whitespace()
        .nth(1) // Get second word
        .and_then(|v| v.strip_prefix('v')) // Remove 'v' prefix
        .ok_or_else(|| eyre::eyre!("could not parse version from fly output"))?;

    Ok(version.to_string())
}

/// Establish starts the daemon, if necessary, and returns a client to it.
pub async fn establish(
    request_builder_graphql: &RequestBuilderGraphql,
    app_name: String,
) -> RdrResult<Client> {
    // First prune invalid peers
    wireguard::prune_invalid_peers(request_builder_graphql, app_name)
        .await
        .wrap_err("failed to prune invalid peers")?;

    let mut client = Client::new(
        "ipc".to_string(),
        path_to_socket(),
        None, // No token initially
    );

    info!("establishing the daemon.");

    // Try pinging existing daemon
    match client.ping().await {
        Ok(res) => {
            info!("pinging the agent succeeded.");
            let current_version = get_current_version().await?;

            if current_version == res.version {
                return Ok(client);
            }

            // Version mismatch - log warning
            let msg = format!(
                "The running flyctl agent (v{}) is older than the current flyctl (v{}).",
                res.version, current_version
            );
            warn!("{}", msg);

            if !res.background {
                return Ok(client);
            }

            // Stop old agent if running in background
            let stop_msg = "The out-of-date agent will be shut down along with existing wireguard connections. The new agent will start automatically as needed.";
            info!("{}", stop_msg);

            if let Err(e) = client.kill().await {
                let kill_err = format!("failed stopping agent: {}", e);
                error!("Error killing the existing agent: {}", kill_err);
                return Err(eyre::eyre!(kill_err));
            }

            // this is gross, but we need to wait for the agent to exit
            tokio::time::sleep(Duration::from_secs(1)).await;

            start_daemon().await
        }

        Err(err) => {
            error!("pinging the agent is failed with err: {err}");
            start_daemon().await
        }
    }
}

impl Client {
    fn new(network: String, addr: PathBuf, token: Option<String>) -> Self {
        Self {
            network,
            address: addr,
            token,
            agent_refused_tokens: false,
        }
    }

    //TODO: Support pre-2017 Windows versions using Named Pipes as flyctl does.
    async fn dial_context(&self) -> Result<Arc<Mutex<NativeIpcStream>>, io::Error> {
        info!("running dial context.");
        let stream = connect_ipc(&self.address).await.map_err(|e| {
            error!("failed to connect to agent with err: {}", e);
            io::Error::new(
                io::ErrorKind::Other,
                format!("failed to connect to agent: {}", e),
            )
        })?;
        Ok(Arc::new(Mutex::new(stream)))
    }

    fn do_<'a, F, Fut, T>(&'a mut self, f: F) -> BoxFuture<'a, RdrResult<T>>
    where
        F: FnOnce(Arc<Mutex<NativeIpcStream>>) -> Fut + Clone + Send + 'a,
        Fut: Future<Output = RdrResult<T>> + Send + 'a,
        T: Send,
    {
        Box::pin(async move {
            if self.agent_refused_tokens {
                return self.do_no_tokens(f).await;
            }
            let token = self.token.clone();
            if let Some(token) = token {
                let should_retry = Arc::new(AtomicBool::new(false));
                let should_retry_clone = should_retry.clone();
                let f_clone = f.clone();
                let result = self
                    .do_no_tokens(|stream| async move {
                        proto::write(stream.clone(), "set-token", &["str", &token]).await?;
                        let data = proto::read(stream.clone()).await?;
                        match data.as_slice() {
                            b"ok" => f_clone(stream).await,
                            _ if is_error(&data) => {
                                error!(
                                    "reading from the agent socket failed with err: {}",
                                    extract_error(&data)
                                );
                                should_retry_clone.store(true, Ordering::SeqCst);
                                Err(extract_error(&data))
                            }
                            err => {
                                error!("reading from the agent socket failed with err: {:#?}", err);
                                Err(eyre::eyre!("invalid response"))
                            }
                        }
                    })
                    .await;
                if should_retry.load(Ordering::SeqCst) {
                    self.agent_refused_tokens = true;
                    return self.do_(f).await;
                }
                result
            } else {
                self.do_no_tokens(f).await
            }
        })
    }

    async fn do_no_tokens<F, Fut, T>(&self, f: F) -> RdrResult<T>
    where
        F: FnOnce(Arc<Mutex<NativeIpcStream>>) -> Fut + Clone,
        Fut: Future<Output = RdrResult<T>>,
    {
        info!("running the agent no tokens middleware.");
        let stream = self.dial_context().await?;

        f(stream.clone()).await

        //INFO: We don't perform a shutdown on the socket here because agent server seems to handle closing the
        //conn anyways and we rely on that.
        //
        // let mut stream = stream.lock().await;
        // info!("trying to shutdown!!");
        // stream.shutdown().await?;
    }

    pub async fn kill(&mut self) -> RdrResult<()> {
        self.do_(|stream| async move {
            proto::write(stream, "kill", &[]).await?;
            Ok(())
        })
        .await
    }

    pub async fn ping(&mut self) -> RdrResult<PingResponse> {
        self.do_(|stream| async move {
            info!("pinging agent.");
            proto::write(stream.clone(), "ping", &[]).await?;

            let data = proto::read(stream).await?;

            if is_ok(&data) {
                let json_data = extract_ok(&data);
                info!(
                    "ping response - raw JSON data: {:?}",
                    String::from_utf8_lossy(json_data)
                );
                serde_json::from_slice(json_data).wrap_err("failed to parse ping response")
            } else {
                info!("error pinging agent.");
                Err(eyre::eyre!("invalid response"))
            }
        })
        .await
    }

    async fn do_establish(
        &mut self,
        slug: &str,
        reestablish: bool,
        network: &str,
    ) -> RdrResult<EstablishResponse> {
        self.do_(|stream| async move {
            let verb = if reestablish {
                "reestablish"
            } else {
                "establish"
            };
            proto::write(stream.clone(), verb, &[slug, network]).await?;

            let data = proto::read(stream).await?;

            if is_ok(&data) {
                let json_data = extract_ok(&data);
                info!(
                    "establish response - raw JSON data: {:?}",
                    String::from_utf8_lossy(json_data)
                );
                serde_json::from_slice(json_data).wrap_err("failed to parse establish response")
            } else if is_error(&data) {
                Err(extract_error(&data))
            } else {
                Err(eyre::eyre!("invalid response"))
            }
        })
        .await
    }

    pub async fn establish(&mut self, slug: &str, network: &str) -> RdrResult<EstablishResponse> {
        self.do_establish(slug, false, network).await
    }

    pub async fn reestablish(&mut self, slug: &str, network: &str) -> RdrResult<EstablishResponse> {
        self.do_establish(slug, true, network).await
    }

    pub async fn probe(&mut self, slug: &str, network: &str) -> RdrResult<()> {
        self.do_(|stream| async move {
            proto::write(stream.clone(), "probe", &[slug, network]).await?;

            let data = proto::read(stream).await?;

            if data == b"ok" {
                Ok(())
            } else if is_error(&data) {
                Err(extract_error(&data))
            } else {
                Err(eyre::eyre!("invalid response"))
            }
        })
        .await
    }

    pub async fn resolve(
        &mut self,
        slug: &str,
        host: &str,
        network: &str,
    ) -> RdrResult<Option<String>> {
        self.do_(|stream| async move {
            proto::write(stream.clone(), "resolve", &[slug, host, network]).await?;

            let data = proto::read(stream).await?;

            if data == b"ok" {
                Err(eyre::eyre!(AgentError::NoSuchHost))
            } else if is_ok(&data) {
                let addr = String::from_utf8_lossy(extract_ok(&data)).into_owned();
                Ok(Some(addr))
            } else if is_error(&data) {
                Err(extract_error(&data))
            } else {
                Err(eyre::eyre!("invalid response"))
            }
        })
        .await
    }

    pub async fn lookup_txt(&mut self, slug: &str, host: &str) -> RdrResult<Vec<String>> {
        self.do_(|stream| async move {
            proto::write(stream.clone(), "lookupTxt", &[slug, host]).await?;

            let data = proto::read(stream).await?;

            if is_ok(&data) {
                let json_data = extract_ok(&data);
                serde_json::from_slice(json_data).wrap_err("failed to parse lookup text response")
            } else if is_error(&data) {
                Err(extract_error(&data))
            } else {
                Err(eyre::eyre!("invalid response"))
            }
        })
        .await
    }

    pub async fn wait_for_tunnel(&mut self, slug: &str, network: &str) -> RdrResult<()> {
        let timeout = Duration::from_secs(4 * 60); // 4 minutes like Go code
        let mut interval = tokio::time::interval(CYCLE);

        let start = time::Instant::now();
        loop {
            match self.probe(slug, network).await {
                Ok(_) => {
                    info!("succeeded waiting for tunnel.");
                    return Ok(());
                }
                Err(err) => {
                    if !matches!(err.downcast_ref(), Some(AgentError::TunnelUnavailable)) {
                        return Err(err);
                    }
                }
            }

            // Check if we've exceeded timeout
            if start.elapsed() >= timeout {
                return Err(eyre::eyre!(AgentError::TunnelUnavailable));
            }

            interval.tick().await;
        }
    }

    // if command run without quiet, give feedback
    pub async fn wait_for_dns(&mut self, slug: &str, host: &str, network: &str) -> RdrResult<()> {
        let timeout = Duration::from_secs(4 * 60); // 4 minutes like Go code
        let mut interval = tokio::time::interval(CYCLE);

        let start = time::Instant::now();
        loop {
            match self.resolve(slug, host, network).await {
                Ok(_) => return Ok(()),
                Err(err) => {
                    // If error is not NoSuchHost, return it
                    if !matches!(err.downcast_ref(), Some(AgentError::NoSuchHost)) {
                        return Err(err);
                    }
                }
            }

            // Check if we've exceeded timeout
            if start.elapsed() >= timeout {
                return Err(eyre::eyre!(AgentError::NoSuchHost));
            }

            interval.tick().await;
        }
    }

    pub async fn dialer(&mut self, slug: &str, network: &str) -> RdrResult<ClientDialer> {
        let er = self.establish(slug, network).await?;

        match er.wireguard_state {
            Some(state) => Ok(ClientDialer {
                slug: slug.to_string(),
                network: network.to_string(),
                timeout: Duration::from_secs(30), // default timeout
                state,
                client: self.clone(),
            }),
            _ => Err(eyre::eyre!("missing WireGuard state in response")),
        }
    }

    pub async fn connect_to_tunnel(
        &mut self,
        slug: &str,
        network: &str,
        silent: bool,
    ) -> RdrResult<ClientDialer> {
        let dialer = self.dialer(slug, network).await?;

        if !silent {
            // TODO: Implement progress indicator
            // warn!("Opening a wireguard tunnel to {}", slug);
        }

        if let Err(err) = self.wait_for_tunnel(slug, network).await {
            return Err(eyre::eyre!(
                "tunnel unavailable for organization {}: {}",
                slug,
                err
            ));
        }

        Ok(dialer)
    }
    pub async fn instances(
        &mut self,
        request_builder_graphql: &RequestBuilderGraphql,
        org: &str,
        app: &str,
    ) -> RdrResult<Instances> {
        // Create channels for both API calls
        let (agent_tx, agent_rx) = tokio::sync::oneshot::channel();
        let (gql_tx, gql_rx) = tokio::sync::oneshot::channel();

        // Spawn agent query
        let org_clone = org.to_string();
        let app_clone = app.to_string();
        let mut client_clone = self.clone();
        tokio::spawn(async move {
            let result = client_clone
                .do_(|stream| async move {
                    proto::write(stream.clone(), "instances", &[&org_clone, &app_clone]).await?;
                    let data = proto::read(stream).await?;

                    if is_ok(&data) {
                        let json_data = extract_ok(&data);
                        serde_json::from_slice(json_data)
                            .wrap_err("failed to parse instances response")
                    } else if is_error(&data) {
                        Err(extract_error(&data))
                    } else {
                        Err(eyre::eyre!("invalid response"))
                    }
                })
                .await;
            agent_tx.send(result).ok();
        })
        .await?;

        // Spawn GQL query
        let app_clone = app.to_string();
        let request_builder_graphql_clone = request_builder_graphql.clone();
        tokio::spawn(async move {
            let result = gql_get_instances(&request_builder_graphql_clone, app_clone).await;
            gql_tx.send(result).ok();
        });

        // Wait for both results
        let agent_result = agent_rx.await.wrap_err("agent task failed")?;
        let gql_result = gql_rx.await.wrap_err("gql task failed")?;

        compare_and_choose_results(gql_result, agent_result, org, app)
    }
}

pub async fn default_client() -> RdrResult<Client> {
    //INFO: Tokens are held in the context as Config in fly code.
    let token = read_access_token().await.ok();
    dial("ipc".to_string(), path_to_socket(), token).await
}

pub async fn dial(network: String, addr: PathBuf, token: Option<String>) -> RdrResult<Client> {
    let mut client = Client::new(network, addr, token);

    match client.ping().await {
        Ok(_) => Ok(client),
        Err(e) => {
            let io_err = e.downcast_ref::<io::Error>();
            match io_err {
                Some(e) if e.kind() == io::ErrorKind::NotFound => {
                    Err(eyre::eyre!(AGENT_NOT_RUNNING))
                }
                _ => Err(e).wrap_err("failed to connect to agent"),
            }
        }
    }
}

#[derive(Clone, Debug)]
pub struct ClientDialer {
    slug: String,
    network: String,
    timeout: Duration,
    pub state: WireGuardState,
    client: Client,
}

impl ClientDialer {
    pub fn new(slug: String, network: String, state: WireGuardState, client: Client) -> Self {
        Self {
            slug,
            network,
            timeout: Duration::from_secs(30),
            state,
            client,
        }
    }
}

impl async_nats_flyradar::Dialer for ClientDialer {
    fn dial(
        &self,
        addr: String,
    ) -> Pin<Box<dyn Future<Output = Result<IpcStreamWrapper, io::Error>> + Send + '_>> {
        Box::pin(async move {
            info!("Starting dial for addr: {}", addr);

            let stream = self.client.dial_context().await?;

            info!(
                "Sending connect command with slug={}, addr={}, timeout={}, network={}",
                self.slug,
                addr,
                self.timeout.as_millis(),
                self.network
            );

            proto::write(stream.clone(), "probe", &[&self.slug, &self.network]).await?;

            let data = proto::read(stream.clone()).await?;

            if data == b"ok" {
                info!("probe succeded");
            } else if is_error(&data) {
                info!("probe err");
            } else {
                info!("probe invalid resp");
            };

            info!("this is the addr: {}", addr);

            let stream = self.client.dial_context().await?;
            proto::write(
                stream.clone(),
                "connect",
                &[
                    &self.slug,
                    &addr,
                    &self.timeout.as_millis().to_string(),
                    &self.network,
                ],
            )
            .await?;

            // Read response to verify connection
            let data = proto::read(stream.clone()).await?;
            match data.as_slice() {
                b"ok" => {
                    let stream_wrapper = IpcStreamWrapper::new(stream);
                    Ok(stream_wrapper)
                }
                err => {
                    info!(
                        "verify connection response - raw JSON data: {:?}",
                        String::from_utf8_lossy(err)
                    );
                    Err(io::Error::new(io::ErrorKind::Other, "connection failed"))
                }
            }
        })
    }
}

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "src/agent/query/gql_get_instances_schema.graphql",
    query_path = "src/agent/query/gql_get_instances.graphql",
    response_derives = "Debug"
)]
pub struct GqlGetInstances;
#[instrument(err)]
pub async fn gql_get_instances(
    request_builder_graphql: &RequestBuilderGraphql,
    app_name: String,
) -> RdrResult<Instances> {
    let variables = gql_get_instances::Variables { app_name };
    let request_body = GqlGetInstances::build_query(variables);
    let response = request_builder_graphql
        .query()
        .json(&request_body)
        .send()
        .await?;

    let bytes = response.bytes().await?;
    let response_body: Response<gql_get_instances::ResponseData> =
        serde_path_to_error::deserialize(&mut serde_json::Deserializer::from_slice(&bytes))?;
    if let Some(errors) = response_body.errors {
        return Err(eyre::eyre!(
            "{}",
            errors
                .iter()
                .map(|e| e.message.clone())
                .collect::<Vec<_>>()
                .join(", "),
        ));
    }

    let mut result = Instances {
        labels: Vec::new(),
        addresses: Vec::new(),
    };
    if let Some(response) = response_body.data {
        // Process allocations
        for alloc in &response.app.allocations {
            if let Ok(ip) = alloc.private_ip.parse::<IpAddr>() {
                result.addresses.push(ip.to_string());
                result
                    .labels
                    .push(format!("{}.{}.internal", alloc.region, response.app.name));
            }
        }

        // Process machines
        for machine in &response.app.machines.nodes {
            if machine.state != "started" {
                continue;
            }

            for ip in &machine.ips.nodes {
                if ip.kind == "privatenet" && ip.family == "v6" {
                    if let Ok(addr) = ip.ip.parse::<IpAddr>() {
                        result.addresses.push(addr.to_string());
                        result
                            .labels
                            .push(format!("{}.{}.internal", machine.region, response.app.name));
                    }
                }
            }
        }

        // Add IP addresses to labels if there's more than one
        if result.addresses.len() > 1 {
            for i in 0..result.addresses.len() {
                result.labels[i] = format!("{} ({})", result.labels[i], result.addresses[i]);
            }
        }
    }
    Ok(result)
}

fn compare_and_choose_results(
    gql_result: RdrResult<Instances>,
    agent_result: RdrResult<Instances>,
    org_slug: &str,
    app_name: &str,
) -> RdrResult<Instances> {
    match (gql_result, agent_result) {
        (Err(gql_err), Err(agent_err)) => {
            // Log both errors
            error!(
                "Two errors looking up: {} {}: gqlErr: {} agentErr: {}",
                org_slug, app_name, gql_err, agent_err
            );
            Err(gql_err)
        }
        (Err(_), Ok(agent)) => {
            // Log GQL error
            error!("GQL error looking up: {} {}", org_slug, app_name);
            Ok(agent)
        }
        (Ok(gql), Err(_)) => {
            // Log agent error
            error!("DNS error looking up: {} {}", org_slug, app_name);
            Ok(gql)
        }
        (Ok(gql), Ok(_agent)) => Ok(gql),
    }
}
