#![forbid(unsafe_code)]

mod defaults;
mod defaults_impl;
mod impls;
mod reload;

use std::collections::BTreeMap;

use defaults::*;
use serde::{Deserialize, Serialize};

pub use reload::ReloadingDataPlaneConfig;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataPlaneConfig {
    pub node_id: String,
    pub cluster: String,
    pub control_plane_addr: String,
    pub admin_addr: String,
    #[serde(default)]
    pub log: LogConfig,
    #[serde(default)]
    pub access_log: AccessLogConfig,
    #[serde(default)]
    pub admin_auth: AdminAuthConfig,
    #[serde(default)]
    pub runtime: RuntimeConfig,
    #[serde(default)]
    pub session_persistence: SessionPersistenceConfig,
    #[serde(default)]
    pub xds_tls: XdsTlsConfig,
    #[serde(default)]
    pub xds_transport: XdsTransportConfig,
    #[serde(default = "default_xds_protocol")]
    pub xds_protocol: String,
    #[serde(default)]
    pub runtime_protection: RuntimeProtectionConfig,
    #[serde(default)]
    pub runtime_tuning: RuntimeTuningConfig,
    #[serde(default)]
    pub experimental: ExperimentalConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogConfig {
    #[serde(default = "default_level")]
    pub level: String,
    #[serde(default = "default_log_format")]
    pub format: String,
    #[serde(default)]
    pub add_source: bool,
    #[serde(default)]
    pub include_target: bool,
    #[serde(default)]
    pub include_thread_ids: bool,
    #[serde(default)]
    pub include_thread_names: bool,
    #[serde(default = "default_log_non_blocking")]
    pub non_blocking: bool,
    #[serde(default = "default_log_non_blocking_buffered_lines")]
    pub non_blocking_buffered_lines: usize,
    #[serde(default = "default_log_drop_when_full")]
    pub drop_when_full: bool,
    #[serde(default)]
    pub open_telemetry: OpenTelemetryConfig,
    #[serde(default)]
    pub sentry: SentryConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenTelemetryConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub endpoint: String,
    #[serde(default = "default_open_telemetry_protocol")]
    pub protocol: String,
    #[serde(default = "default_open_telemetry_timeout_ms")]
    pub timeout_ms: u64,
    #[serde(default)]
    pub insecure: bool,
    #[serde(default = "default_open_telemetry_sample_ratio")]
    pub sample_ratio: f64,
    #[serde(default = "default_open_telemetry_service_name")]
    pub service_name: String,
    #[serde(default)]
    pub service_namespace: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SentryConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub dsn: String,
    #[serde(default)]
    pub environment: String,
    #[serde(default = "default_sentry_sample_rate")]
    pub sample_rate: f32,
    #[serde(default = "default_sentry_traces_sample_rate")]
    pub traces_sample_rate: f32,
    #[serde(default = "default_true")]
    pub attach_stacktrace: bool,
    #[serde(default)]
    pub send_default_pii: bool,
    #[serde(default)]
    pub debug: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccessLogConfig {
    #[serde(default = "default_access_enabled")]
    pub enabled: bool,
    #[serde(default = "default_access_path")]
    pub path: String,
    #[serde(default = "default_access_format")]
    pub format: String,
    #[serde(default = "default_access_mode")]
    pub mode: String,
    #[serde(default)]
    pub formats: BTreeMap<String, String>,
    #[serde(default)]
    pub format_name: String,
    #[serde(default = "default_access_sample_rate")]
    pub sample_rate: f64,
    #[serde(default = "default_route_annotation_prefix")]
    pub route_annotation_prefix: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AdminAuthConfig {
    #[serde(default)]
    pub bearer_token: String,
    #[serde(default)]
    pub bearer_token_file: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeConfig {
    #[serde(default = "default_http_listen_addr")]
    pub http_listen_addr: String,
    #[serde(default)]
    pub enable_ipv6: bool,
    #[serde(default = "default_http3_enabled")]
    pub enable_http3: bool,
    #[serde(default = "default_tls_min")]
    pub tls_min_version: String,
    #[serde(default = "default_tls_max")]
    pub tls_max_version: String,
    #[serde(default)]
    pub tls_asset_dir: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SessionPersistenceConfig {
    #[serde(default)]
    pub secret_key: String,
    #[serde(default)]
    pub secret_key_file: String,
    #[serde(default)]
    pub shared_secret: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct XdsTlsConfig {
    #[serde(default = "default_xds_tls_enabled")]
    pub enabled: bool,
    #[serde(default)]
    pub ca_path: String,
    #[serde(default)]
    pub cert_path: String,
    #[serde(default)]
    pub key_path: String,
    #[serde(default)]
    pub domain_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct XdsTransportConfig {
    #[serde(default = "default_xds_connect_timeout_ms")]
    pub connect_timeout_ms: u64,
    #[serde(default = "default_xds_keepalive_interval_ms")]
    pub keepalive_interval_ms: u64,
    #[serde(default = "default_xds_keepalive_timeout_ms")]
    pub keepalive_timeout_ms: u64,
    #[serde(default = "default_xds_initial_reconnect_backoff_ms")]
    pub initial_reconnect_backoff_ms: u64,
    #[serde(default = "default_xds_max_reconnect_backoff_ms")]
    pub max_reconnect_backoff_ms: u64,
    #[serde(default = "default_xds_apply_timeout_ms")]
    pub apply_timeout_ms: u64,
    #[serde(default = "default_xds_apply_poll_interval_ms")]
    pub apply_poll_interval_ms: u64,
    #[serde(default = "default_xds_stale_stream_timeout_ms")]
    pub stale_stream_timeout_ms: u64,
    #[serde(default = "default_xds_snapshot_freshness_timeout_ms")]
    pub snapshot_freshness_timeout_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeProtectionConfig {
    #[serde(default)]
    pub http_global_inflight_limit: usize,
    #[serde(default)]
    pub http_listener_inflight_limit: usize,
    #[serde(default)]
    pub http_route_inflight_limit: usize,
    #[serde(default)]
    pub http_backend_circuit_breaker_max_requests: usize,
    #[serde(default)]
    pub http_global_rate_limit_requests_per_second: u32,
    #[serde(default)]
    pub http_global_rate_limit_burst: u32,
    #[serde(default)]
    pub http_listener_rate_limit_requests_per_second: u32,
    #[serde(default)]
    pub http_listener_rate_limit_burst: u32,
    #[serde(default)]
    pub http_route_rate_limit_requests_per_second: u32,
    #[serde(default)]
    pub http_route_rate_limit_burst: u32,
    #[serde(default = "default_http_max_request_body_bytes")]
    pub http_max_request_body_bytes: usize,
    #[serde(default = "default_http_max_request_header_bytes")]
    pub http_max_request_header_bytes: usize,
    #[serde(default)]
    pub tcp_global_connection_limit: usize,
    #[serde(default)]
    pub tcp_listener_connection_limit: usize,
    #[serde(default)]
    pub udp_global_datagram_limit: usize,
    #[serde(default)]
    pub udp_listener_datagram_limit: usize,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HttpCapacityConfig {
    #[serde(default)]
    pub worker_threads: usize,
    #[serde(default)]
    pub accept_concurrency: usize,
    #[serde(default)]
    pub upstream_keepalive_pool_size: usize,
    #[serde(default)]
    pub reuse_port: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeTuningConfig {
    #[serde(default = "default_http_reload_retry_interval_ms")]
    pub http_reload_retry_interval_ms: u64,
    #[serde(default = "default_stream_reload_retry_interval_ms")]
    pub stream_reload_retry_interval_ms: u64,
    #[serde(default = "default_downstream_read_timeout_ms")]
    pub downstream_read_timeout_ms: u64,
    #[serde(default = "default_http_max_connection_age_ms")]
    pub http_max_connection_age_ms: u64,
    #[serde(default = "default_http_keepalive_request_limit")]
    pub http_keepalive_request_limit: u32,
    #[serde(default)]
    pub http_capacity: HttpCapacityConfig,
    #[serde(default = "default_request_mirror_max_concurrency")]
    pub request_mirror_max_concurrency: usize,
    #[serde(default = "default_udp_response_idle_timeout_ms")]
    pub udp_response_idle_timeout_ms: u64,
    #[serde(default = "default_tcp_proxy_buffer_bytes")]
    pub tcp_proxy_buffer_bytes: usize,
    #[serde(default = "default_tcp_session_idle_timeout_ms")]
    pub tcp_session_idle_timeout_ms: u64,
    #[serde(default = "default_tcp_max_connection_age_ms")]
    pub tcp_max_connection_age_ms: u64,
    #[serde(default = "default_retry_budget_enabled")]
    pub retry_budget_enabled: bool,
    #[serde(default = "default_retry_budget_ratio_percent")]
    pub retry_budget_ratio_percent: u32,
    #[serde(default = "default_retry_budget_burst")]
    pub retry_budget_burst: u32,
    #[serde(default = "default_stream_upstream_pool_size")]
    pub stream_upstream_pool_size: usize,
    #[serde(default = "default_stream_upstream_pool_idle_timeout_ms")]
    pub stream_upstream_pool_idle_timeout_ms: u64,
    #[serde(default = "default_graceful_drain_period_ms")]
    pub graceful_drain_period_ms: u64,
    #[serde(default = "default_active_health_check_enabled")]
    pub active_health_check_enabled: bool,
    #[serde(default = "default_active_health_check_interval_ms")]
    pub active_health_check_interval_ms: u64,
    #[serde(default = "default_active_health_check_timeout_ms")]
    pub active_health_check_timeout_ms: u64,
    #[serde(default = "default_active_health_check_unhealthy_threshold")]
    pub active_health_check_unhealthy_threshold: u32,
    #[serde(default)]
    pub downstream_tcp_keepalive: TcpKeepaliveConfig,
    #[serde(default)]
    pub upstream_tcp_keepalive: TcpKeepaliveConfig,
    #[serde(default = "default_work_stealing")]
    pub work_stealing: bool,
    #[serde(default)]
    pub downstream_tcp_fastopen: Option<usize>,
    #[serde(default)]
    pub downstream_dscp: Option<u8>,
    #[serde(default)]
    pub upstream_tcp_recv_buf: usize,
    #[serde(default = "default_upstream_tcp_fast_open")]
    pub upstream_tcp_fast_open: bool,
    #[serde(default = "default_upstream_connection_timeout_ms")]
    pub upstream_connection_timeout_ms: u64,
    #[serde(default = "default_upstream_read_timeout_ms")]
    pub upstream_read_timeout_ms: u64,
    #[serde(default = "default_upstream_idle_timeout_ms")]
    pub upstream_idle_timeout_ms: u64,
    #[serde(default)]
    pub upstream_dscp: Option<u8>,
    #[serde(default)]
    pub http_cache: HttpCacheConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HttpCacheConfig {
    #[serde(default = "default_http_cache_enabled")]
    pub enabled: bool,
    #[serde(default = "default_http_cache_max_size_mb")]
    pub max_size_mb: usize,
    #[serde(default = "default_http_cache_max_entry_size_mb")]
    pub max_entry_size_mb: usize,
    #[serde(default = "default_http_cache_default_ttl_seconds")]
    pub default_ttl_seconds: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExperimentalConfig {
    #[serde(default)]
    pub enable_experimental_gateway: bool,
    #[serde(default)]
    pub enable_ai_gateway: bool,
    #[serde(default = "default_ai_gateway_max_request_body_bytes")]
    pub ai_gateway_max_request_body_bytes: usize,
    #[serde(default = "default_wasm_max_concurrency")]
    pub wasm_max_concurrency: usize,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RoutePolicyConfig {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timeout: Option<RoutePolicyTimeoutConfig>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub body_limit: Option<RoutePolicyBodyLimitConfig>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub proxy: Option<RoutePolicyProxyConfig>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub connection: Option<RoutePolicyConnectionConfig>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RoutePolicyTimeoutConfig {
    #[serde(default)]
    pub request: Option<u64>,
    #[serde(default)]
    pub backend_request: Option<u64>,
    #[serde(default)]
    pub connect: Option<u64>,
    #[serde(default)]
    pub next_upstream: Option<u64>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RoutePolicyBodyLimitConfig {
    #[serde(default)]
    pub max_request_body_bytes: Option<usize>,
    #[serde(default)]
    pub request_body_buffer_bytes: Option<usize>,
    #[serde(default)]
    pub max_request_header_bytes: Option<usize>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RoutePolicyProxyConfig {
    #[serde(default)]
    pub request_buffering: Option<bool>,
    #[serde(default)]
    pub response_buffering: Option<bool>,
    #[serde(default)]
    pub buffer_size: Option<usize>,
    #[serde(default)]
    pub buffer_count: Option<u32>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RoutePolicyConnectionConfig {
    #[serde(default)]
    pub keepalive_requests: Option<u32>,
    #[serde(default)]
    pub keepalive_time: Option<u64>,
    #[serde(default)]
    pub keepalive_timeout: Option<u64>,
    #[serde(default)]
    pub upstream_keepalive_pool_size: Option<u32>,
    #[serde(default)]
    pub upstream_keepalive_idle: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TcpKeepaliveConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default = "default_tcp_keepalive_idle_ms")]
    pub idle_ms: u64,
    #[serde(default = "default_tcp_keepalive_interval_ms")]
    pub interval_ms: u64,
    #[serde(default = "default_tcp_keepalive_probe_count")]
    pub probe_count: usize,
    #[serde(default)]
    pub user_timeout_ms: u64,
}

#[cfg(test)]
mod tests;
