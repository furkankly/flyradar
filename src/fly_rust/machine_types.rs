use std::collections::HashMap;
use std::time::Duration;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Deserializer, Serialize};

pub const MACHINE_CONFIG_METADATA_KEY_FLY_MANAGED_POSTGRES: &str = "fly-managed-postgres";
pub const MACHINE_CONFIG_METADATA_KEY_FLY_PLATFORM_VERSION: &str = "fly_platform_version";
pub const MACHINE_CONFIG_METADATA_KEY_FLY_RELEASE_ID: &str = "fly_release_id";
pub const MACHINE_CONFIG_METADATA_KEY_FLY_RELEASE_VERSION: &str = "fly_release_version";
pub const MACHINE_CONFIG_METADATA_KEY_FLY_PROCESS_GROUP: &str = "fly_process_group";
pub const MACHINE_CONFIG_METADATA_KEY_FLY_PREVIOUS_ALLOC: &str = "fly_previous_alloc";
pub const MACHINE_CONFIG_METADATA_KEY_FLYCTL_VERSION: &str = "fly_flyctl_version";
pub const MACHINE_CONFIG_METADATA_KEY_FLYCTL_BG_TAG: &str = "fly_bluegreen_deployment_tag";
pub const MACHINE_FLY_PLATFORM_VERSION_2: &str = "v2";
pub const MACHINE_PROCESS_GROUP_APP: &str = "app";
pub const MACHINE_PROCESS_GROUP_FLY_APP_RELEASE_COMMAND: &str = "fly_app_release_command";
pub const MACHINE_PROCESS_GROUP_FLY_APP_TEST_MACHINE_COMMAND: &str = "fly_app_test_machine_command";
pub const MACHINE_PROCESS_GROUP_FLY_APP_CONSOLE: &str = "fly_app_console";
pub const MACHINE_STATE_DESTROYED: &str = "destroyed";
pub const MACHINE_STATE_DESTROYING: &str = "destroying";
pub const MACHINE_STATE_STARTED: &str = "started";
pub const MACHINE_STATE_STOPPED: &str = "stopped";
pub const MACHINE_STATE_SUSPENDED: &str = "suspended";
pub const MACHINE_STATE_CREATED: &str = "created";
pub const DEFAULT_VM_SIZE: &str = "shared-cpu-1x";
pub const DEFAULT_GPU_VM_SIZE: &str = "performance-8x";

#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum HostStatus {
    Ok,
    Unknown,
    Unreachable,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Machine {
    pub id: String,
    pub name: String,
    pub state: String,
    pub region: String,
    pub image_ref: MachineImageRef,
    pub instance_id: String,
    pub version: Option<String>,
    pub private_ip: String,
    pub created_at: String,
    pub updated_at: String,
    pub config: Option<MachineConfig>,
    pub events: Option<Vec<MachineEvent>>,
    pub checks: Option<Vec<MachineCheckStatus>>,
    #[serde(rename = "nonce")]
    pub lease_nonce: Option<String>,
    pub host_status: HostStatus,
    pub incomplete_config: Option<MachineConfig>,
}

impl Machine {
    pub fn full_image_ref(&self) -> String {
        let mut img_str = format!("{}/{}", self.image_ref.registry, self.image_ref.repository);
        if !self.image_ref.tag.is_empty() && !self.image_ref.digest.is_empty() {
            img_str = format!(
                "{}:{}@{}",
                img_str, self.image_ref.tag, self.image_ref.digest
            );
        } else if !self.image_ref.digest.is_empty() {
            img_str = format!("{}@{}", img_str, self.image_ref.digest);
        } else if !self.image_ref.tag.is_empty() {
            img_str = format!("{}:{}", img_str, self.image_ref.tag);
        }
        img_str
    }

    pub fn image_ref_with_version(&self) -> String {
        let mut ref_str = format!("{}:{}", self.image_ref.repository, self.image_ref.tag);
        if let Some(labels) = &self.image_ref.labels {
            if let Some(version) = labels.get("fly.version") {
                ref_str = format!("{} ({})", ref_str, version);
            }
        }
        ref_str
    }

    // GetConfig returns `IncompleteConfig` if `Config` is unset which happens when
    // `HostStatus` isn't "ok"
    pub fn get_config(&self) -> Option<&MachineConfig> {
        self.config.as_ref().or(self.incomplete_config.as_ref())
    }

    pub fn get_metadata_by_key(&self, key: &str) -> String {
        self.get_config()
            .and_then(|c| c.metadata.as_ref()?.get(key).cloned())
            .unwrap_or_default()
    }

    pub fn is_apps_v2(&self) -> bool {
        self.get_metadata_by_key(MACHINE_CONFIG_METADATA_KEY_FLY_PLATFORM_VERSION)
            == MACHINE_FLY_PLATFORM_VERSION_2
    }

    pub fn is_fly_apps_platform(&self) -> bool {
        self.is_apps_v2() && self.is_active()
    }

    pub fn is_fly_apps_release_command(&self) -> bool {
        self.is_fly_apps_platform() && self.is_release_command_machine()
    }

    pub fn is_fly_apps_console(&self) -> bool {
        self.is_fly_apps_platform() && self.has_process_group(MACHINE_PROCESS_GROUP_FLY_APP_CONSOLE)
    }

    pub fn is_active(&self) -> bool {
        self.state != MACHINE_STATE_DESTROYING && self.state != MACHINE_STATE_DESTROYED
    }

    pub fn process_group(&self) -> String {
        self.get_config()
            .and_then(|c| {
                c.metadata
                    .as_ref()?
                    .get(MACHINE_CONFIG_METADATA_KEY_FLY_PROCESS_GROUP)
                    .or(c.metadata.as_ref()?.get("process_group"))
            })
            .cloned()
            .unwrap_or_default()
    }

    pub fn has_process_group(&self, desired: &str) -> bool {
        self.process_group() == desired
    }

    pub fn image_version(&self) -> String {
        if let Some(labels) = &self.image_ref.labels {
            return labels.get("fly.version").cloned().unwrap_or_default();
        };
        String::default()
    }

    pub fn image_repository(&self) -> &str {
        &self.image_ref.repository
    }

    pub fn top_level_checks(&self) -> HealthCheckStatus {
        let mut res = HealthCheckStatus::default();
        if let Some(checks) = &self.checks {
            for check in checks {
                if !check.name.starts_with("servicecheck-") {
                    res.total += 1;
                    match check.status {
                        ConsulCheckStatus::Passing => res.passing += 1,
                        ConsulCheckStatus::Warning => res.warn += 1,
                        ConsulCheckStatus::Critical => res.critical += 1,
                    }
                }
            }
        }
        res
    }

    pub fn is_release_command_machine(&self) -> bool {
        self.has_process_group(MACHINE_PROCESS_GROUP_FLY_APP_RELEASE_COMMAND)
            || self.get_metadata_by_key("process_group") == "release_command"
    }
}

#[derive(Clone, Debug, Default)]
pub struct HealthCheckStatus {
    pub total: i32,
    pub passing: i32,
    pub warn: i32,
    pub critical: i32,
}

#[derive(Clone, Deserialize, Debug)]
pub struct MachineImageRef {
    pub registry: String,
    pub repository: String,
    pub tag: String,
    pub digest: String,
    pub labels: Option<HashMap<String, String>>,
}

#[derive(Clone, Deserialize, Debug)]
pub struct MachineEvent {
    pub r#type: String,
    pub status: String,
    pub request: Option<MachineRequest>,
    pub source: String,
    pub timestamp: i64,
}

#[derive(Clone, Deserialize, Debug)]
pub struct MachineRequest {
    pub exit_event: Option<MachineExitEvent>,
    #[serde(rename = "MonitorEvent")]
    pub monitor_event: Option<MachineMonitorEvent>,
    pub restart_count: Option<i32>,
}

#[derive(Clone, Deserialize, Debug)]
pub struct MachineMonitorEvent {
    pub exit_event: Option<MachineExitEvent>,
}

#[derive(Clone, Deserialize, Default, Debug)]
#[serde(default)]
pub struct MachineExitEvent {
    pub exit_code: i32,
    pub guest_exit_code: i32,
    pub guest_signal: i32,
    pub oom_killed: bool,
    pub requested_stop: bool,
    pub restarting: bool,
    pub signal: i32,
    pub exited_at: DateTime<Utc>,
}

#[derive(Debug, Default)]
pub struct StopMachineInput {
    pub id: String,
    pub signal: String,
    pub timeout: Duration,
}
#[derive(Debug, Default)]
pub struct RestartMachineInput {
    pub id: String,
    pub signal: Option<String>,
    pub timeout: Option<Duration>,
    pub force_stop: bool,
    pub skip_health_checks: bool,
}

#[allow(dead_code)]
struct MachineIP {
    family: String,
    kind: String,
    ip: String,
    mask_size: i32,
}

#[derive(Debug)]
pub struct RemoveMachineInput {
    pub id: String,
    pub kill: bool,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum MachineRestartPolicy {
    No,
    OnFailure,
    Always,
    SpotPrice,
}

/// The Machine restart policy defines whether and how flyd restarts a Machine after its main process exits.
/// See https://fly.io/docs/machines/guides-examples/machine-restart-policy/.
#[derive(Clone, Debug, Deserialize)]
pub struct MachineRestart {
    /// - no: Never try to restart a Machine automatically when its main process exits, whether that's on purpose or on a crash.
    /// - always: Always restart a Machine automatically and never let it enter a stopped state, even when the main process exits cleanly.
    /// - on-failure: Try up to MaxRetries times to automatically restart the Machine if it exits with a non-zero exit code. Default when no explicit policy is set, and for Machines with schedules.
    /// - spot-price: Starts the Machine only when there is capacity and the spot price is less than or equal to the bid price.
    pub policy: Option<MachineRestartPolicy>,

    /// When policy is on-failure, the maximum number of times to attempt to restart the Machine before letting it stop.
    pub max_retries: Option<i32>,

    /// GPU bid price for spot Machines.
    pub gpu_bid_price: Option<f32>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct MachineMount {
    pub encrypted: bool,
    pub path: String,
    pub size_gb: i32,
    pub volume: String,
    pub name: String,
    pub extend_threshold_percent: Option<i32>,
    pub add_size_gb: Option<i32>,
    pub size_gb_limit: Option<i32>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct MachineGuest {
    pub cpu_kind: String,
    pub cpus: i32,
    pub memory_mb: i32,
    pub gpus: Option<i32>,
    pub gpu_kind: Option<String>,
    pub host_dedication_id: Option<String>,
    pub kernel_args: Option<Vec<String>>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct MachineMetrics {
    pub port: i32,
    pub path: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MachineCheckKind {
    Informational,
    Readiness,
}

#[derive(Clone, Debug, Deserialize)]
pub struct MachineCheck {
    /// The port to connect to, often the same as internal_port
    pub port: Option<i32>,

    /// tcp or http
    #[serde(rename = "type")]
    pub check_type: Option<String>,

    /// Kind of the check (informational, readiness)
    pub kind: Option<MachineCheckKind>,

    /// The time between connectivity checks
    #[serde(with = "humantime_serde")]
    pub interval: Option<Duration>,

    /// The maximum time a connection can take before being reported as failing its health check
    #[serde(with = "humantime_serde")]
    pub timeout: Option<Duration>,

    /// The time to wait after a VM starts before checking its health
    #[serde(with = "humantime_serde")]
    pub grace_period: Option<Duration>,

    /// For http checks, the HTTP method to use to when making the request
    pub method: Option<String>,

    /// For http checks, the path to send the request to
    pub path: Option<String>,

    /// For http checks, whether to use http or https
    pub protocol: Option<String>,

    /// For http checks with https protocol, whether or not to verify the TLS certificate
    pub tls_skip_verify: Option<bool>,

    /// If the protocol is https, the hostname to use for TLS certificate validation
    pub tls_server_name: Option<String>,

    pub headers: Option<Vec<MachineHTTPHeader>>,
}

/// For http checks, an array of objects with string field Name and array of strings field Values.
/// The key/value pairs specify header and header values that will get passed with the check call.
#[derive(Clone, Debug, Deserialize)]
pub struct MachineHTTPHeader {
    /// The header name
    pub name: String,

    /// The header value
    pub values: Option<Vec<String>>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ConsulCheckStatus {
    Critical,
    Warning,
    Passing,
}

#[derive(Clone, Debug, Deserialize)]
pub struct MachineCheckStatus {
    pub name: String,
    pub status: ConsulCheckStatus,
    pub output: String,
    pub updated_at: Option<DateTime<Utc>>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct MachinePort {
    pub port: Option<i32>,
    pub start_port: Option<i32>,
    pub end_port: Option<i32>,
    pub handlers: Option<Vec<String>>,
    pub force_https: Option<bool>,
    pub tls_options: Option<TLSOptions>,
    pub http_options: Option<HTTPOptions>,
    pub proxy_proto_options: Option<ProxyProtoOptions>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct ProxyProtoOptions {
    pub version: String,
}

#[derive(Clone, Debug, Deserialize)]
pub struct TLSOptions {
    pub alpn: Option<Vec<String>>,
    pub versions: Option<Vec<String>>,
    pub default_self_signed: Option<bool>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct HTTPOptions {
    pub compress: Option<bool>,
    pub response: Option<HTTPResponseOptions>,
    pub h2_backend: Option<bool>,
    pub idle_timeout: Option<u32>,
    pub headers_read_timeout: Option<u32>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct HTTPResponseOptions {
    pub headers: HashMap<String, serde_json::Value>,
    pub pristine: Option<bool>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct MachineService {
    pub protocol: String,
    pub internal_port: i32,
    /// Accepts a string (new format) or a boolean (old format). For backward compatibility with older clients, the API continues to use booleans for "off" and "stop" in responses.
    /// * "off" or false - Do not autostop the Machine.
    /// * "stop" or true - Automatically stop the Machine.
    /// * "suspend" - Automatically suspend the Machine, falling back to a full stop if this is not possible.
    pub autostop: Option<MachineAutostop>,
    pub autostart: Option<bool>,
    pub min_machines_running: Option<i32>,
    pub ports: Option<Vec<MachinePort>>,
    pub checks: Option<Vec<MachineCheck>>,
    pub concurrency: Option<MachineServiceConcurrency>,
    pub force_instance_key: Option<String>,
    pub force_instance_description: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct MachineServiceConcurrency {
    #[serde(rename = "type")]
    pub concurrency_type: String,
    pub hard_limit: i32,
    pub soft_limit: i32,
}

#[derive(Clone, Debug, Deserialize)]
pub struct MachineConfig {
    /// An object filled with key/value pairs to be set as environment variables
    pub env: HashMap<String, String>,
    pub init: MachineInit,
    pub guest: Option<MachineGuest>,
    pub metadata: Option<HashMap<String, String>>,
    pub mounts: Option<Vec<MachineMount>>,
    pub services: Option<Vec<MachineService>>,
    pub metrics: Option<MachineMetrics>,
    pub checks: Option<HashMap<String, MachineCheck>>,
    pub statics: Option<Vec<Option<Static>>>,

    /// The docker image to run
    pub image: String,
    pub files: Option<Vec<Option<File>>>,

    pub schedule: Option<String>,
    /// Optional boolean telling the Machine to destroy itself once it's complete (default false)
    pub auto_destroy: Option<bool>,
    pub restart: Option<MachineRestart>,
    pub dns: Option<DNSConfig>,
    pub processes: Option<Vec<MachineProcess>>,

    /// Standbys enable a machine to be a standby for another. In the event of a hardware failure,
    /// the standby machine will be started.
    pub standbys: Option<Vec<String>>,

    pub stop_config: Option<StopConfig>,

    /// Deprecated: use Guest instead
    #[serde(rename = "VMSize")]
    pub size: Option<String>,
    /// Deprecated: use Service.Autostart instead
    pub disable_machine_autostart: Option<bool>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Static {
    pub guest_path: String,
    pub url_prefix: String,
    pub tigris_bucket: String,
    pub index_document: String,
}

#[derive(Clone, Debug, Deserialize)]
pub struct MachineInit {
    pub exec: Option<Vec<String>>,
    pub entrypoint: Option<Vec<String>>,
    pub cmd: Option<Vec<String>>,
    pub tty: Option<bool>,
    pub swap_size_mb: Option<i32>,
    pub kernel_args: Option<Vec<String>>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct DNSConfig {
    pub skip_registration: bool,
    pub nameservers: Option<Vec<String>>,
    pub searches: Option<Vec<String>>,
    pub options: Option<Vec<DnsOption>>,
    pub dns_forward_rules: Option<Vec<DnsForwardRule>>,
    pub hostname: String,
    pub hostname_fqdn: String,
}

#[derive(Clone, Debug, Deserialize)]
pub struct DnsForwardRule {
    pub basename: String,
    pub addr: String,
}

#[derive(Clone, Debug, Deserialize)]
pub struct DnsOption {
    pub name: String,
    pub value: String,
}

#[derive(Clone, Debug, Deserialize)]
pub struct StopConfig {
    pub timeout: Option<Duration>,
    pub signal: Option<String>,
}

/// A file that will be written to the Machine. One of RawValue or SecretName must be set.
#[derive(Clone, Debug, Deserialize)]
pub struct File {
    /// GuestPath is the path on the machine where the file will be written and must be an absolute path.
    /// For example: /full/path/to/file.json
    pub guest_path: String,

    /// The base64 encoded string of the file contents.
    pub raw_value: Option<String>,

    /// The name of the secret that contains the base64 encoded file contents.
    pub secret_name: Option<String>,

    /// Mode bits used to set permissions on this file as accepted by chmod(2).
    pub mode: u32,
}

#[derive(Clone, Debug, Deserialize)]
pub struct MachineLease {
    pub status: String,
    pub data: Option<MachineLeaseData>,
    pub message: Option<String>,
    pub code: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct MachineLeaseData {
    pub nonce: String,
    pub expires_at: i64,
    pub owner: String,
    pub version: String,
}

#[derive(Clone, Debug, Deserialize)]
pub struct MachineStartResponse {
    pub message: String,
    pub status: String,
    pub previous_state: String,
}

#[derive(Clone, Debug, Deserialize)]
pub struct LaunchMachineInput {
    pub config: Option<MachineConfig>,
    pub region: String,
    pub name: String,
    pub skip_launch: bool,
    pub skip_service_registration: bool,
    pub lsvd: bool,
    pub lease_ttl: i32,
    //INFO: there are some client side only fields in fly-go
    // ID                  string `json:"-"`
    // SkipHealthChecks    bool   `json:"-"`
    // RequiresReplacement bool   `json:"-"`
    // Timeout             int    `json:"-"`
}

#[derive(Clone, Debug, Deserialize)]
pub struct MachineProcess {
    #[serde(rename = "exec")]
    pub exec_override: Option<Vec<String>>,
    #[serde(rename = "entrypoint")]
    pub entrypoint_override: Option<Vec<String>>,
    #[serde(rename = "cmd")]
    pub cmd_override: Option<Vec<String>>,
    #[serde(rename = "user")]
    pub user_override: String,
    pub env: HashMap<String, String>,
    /// Secrets can be provided at the process level to explicitly indicate which secrets should be
    /// used for the process. If not provided, the secrets provided at the machine level will be used.
    pub secrets: Option<Vec<MachineSecret>>,
    /// IgnoreAppSecrets can be set to true to ignore the secrets for the App the Machine belongs to
    /// and only use the secrets provided at the process level. The default/legacy behavior is to use
    /// the secrets provided at the App level.
    pub ignore_app_secrets: bool,

    /// EnvFrom can be provided to set environment variables from machine fields.
    pub env_from: Option<Vec<EnvFrom>>,
}

/// A Secret needing to be set in the environment of the Machine. env_var is required
/// and name can be used to reference a secret name where the environment variable is different
/// from what was set originally using the API. NOTE: When secrets are provided on any process, it
/// will override the secrets provided at the machine level.
#[derive(Clone, Debug, Deserialize)]
pub struct MachineSecret {
    /// EnvVar is required and is the name of the environment variable that will be set from the
    /// secret. It must be a valid environment variable name.
    pub env_var: String,

    /// Name is optional and when provided is used to reference a secret name where the EnvVar is
    /// different from what was set as the secret name.
    pub name: Option<String>,
}

/// EnvVar defines an environment variable to be populated from a machine field, env_var
/// and field_ref are required.
#[derive(Clone, Debug, Deserialize)]
pub struct EnvFrom {
    /// EnvVar is required and is the name of the environment variable that will be set from the
    /// secret. It must be a valid environment variable name.
    pub env_var: String,

    /// FieldRef selects a field of the Machine: supports id, version, app_name, private_ip, region, image.
    //TODO: enum?
    //`json:"field_ref" enums:"id,version,app_name,private_ip,region,image"`
    pub field_ref: String,
}

#[derive(Clone, Debug, Deserialize)]
pub struct MachineExecRequest {
    pub cmd: String,
    pub timeout: i32,
}

#[derive(Clone, Debug, Deserialize)]
pub struct MachineExecResponse {
    pub exit_code: i32,
    pub stdout: String,
    pub stderr: String,
}

pub type MachinePsResponse = Option<Vec<ProcessStat>>;

#[derive(Clone, Debug, Deserialize)]
pub struct ProcessStat {
    pub pid: i32,
    pub stime: u64,
    pub rtime: u64,
    pub command: String,
    pub directory: String,
    pub cpu: u64,
    pub rss: u64,
    pub listen_sockets: Option<Vec<ListenSocket>>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct ListenSocket {
    pub proto: String,
    pub address: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MachineAutostop {
    Off,
    Stop,
    Suspend,
}

impl<'de> Deserialize<'de> for MachineAutostop {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum AutostopInput {
            String(String),
            Bool(bool),
        }

        match AutostopInput::deserialize(deserializer)? {
            AutostopInput::String(s) => match s.to_lowercase().as_str() {
                "off" => Ok(MachineAutostop::Off),
                "stop" => Ok(MachineAutostop::Stop),
                "suspend" => Ok(MachineAutostop::Suspend),
                _ => Err(serde::de::Error::custom(format!(
                    "Invalid autostop value: {}",
                    s
                ))),
            },
            AutostopInput::Bool(b) => {
                if b {
                    Ok(MachineAutostop::Stop)
                } else {
                    Ok(MachineAutostop::Off)
                }
            }
        }
    }
}
