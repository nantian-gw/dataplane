use std::time::Duration;

pub(crate) fn default_level() -> String {
    "info".to_string()
}

pub(crate) fn default_log_format() -> String {
    "json".to_string()
}

pub(crate) fn default_log_non_blocking() -> bool {
    true
}

pub(crate) fn default_log_non_blocking_buffered_lines() -> usize {
    65_536
}

pub(crate) fn default_log_drop_when_full() -> bool {
    true
}

pub(crate) fn default_open_telemetry_protocol() -> String {
    "grpc".to_string()
}

pub(crate) fn default_open_telemetry_timeout_ms() -> u64 {
    3_000
}

pub(crate) fn default_open_telemetry_sample_ratio() -> f64 {
    1.0
}

pub(crate) fn default_open_telemetry_service_name() -> String {
    "nantian-dataplane".to_string()
}

pub(crate) fn default_access_enabled() -> bool {
    true
}

pub(crate) fn default_access_path() -> String {
    "/var/log/nantian-gw/access.log".to_string()
}

pub(crate) fn default_access_format() -> String {
    "%CLIENT_IP% - - [%TIMESTAMP%] \"%REQUEST%\" %STATUS% %BYTES_SENT% \"%REFERER%\" \"%USER_AGENT%\" %LATENCY_MS%ms %UPSTREAM_ADDR% uct=%UPSTREAM_CONNECT_TIME_MS%ms rt=%ROUTE_NAMESPACE%/%ROUTE_NAME% rid=%REQUEST_ID% flags=%RESPONSE_FLAGS% retries=%RETRY_ATTEMPTS% sn=%SNAPSHOT_VERSION%".to_string()
}

pub(crate) fn default_access_mode() -> String {
    "json".to_string()
}

pub(crate) fn default_access_sample_rate() -> f64 {
    0.5
}

pub(crate) fn default_route_annotation_prefix() -> String {
    "gateway.nantian.dev/access-log-".to_string()
}

pub(crate) fn default_http_listen_addr() -> String {
    "0.0.0.0:80".to_string()
}

pub(crate) fn default_tls_min() -> String {
    "1.2".to_string()
}

pub(crate) fn default_tls_max() -> String {
    "1.3".to_string()
}

pub(crate) fn default_http3_enabled() -> bool {
    false
}

pub(crate) fn default_http_reload_retry_interval_ms() -> u64 {
    1_000
}

pub(crate) fn default_stream_reload_retry_interval_ms() -> u64 {
    1_000
}

pub(crate) fn default_downstream_read_timeout_ms() -> u64 {
    60_000
}

pub(crate) fn default_http_keepalive_request_limit() -> u32 {
    1000
}

pub(crate) fn default_http_max_connection_age_ms() -> u64 {
    3_600_000
}

pub(crate) fn default_request_mirror_max_concurrency() -> usize {
    1_024
}

pub(crate) fn default_udp_response_idle_timeout_ms() -> u64 {
    500
}

pub(crate) fn default_tcp_proxy_buffer_bytes() -> usize {
    16 * 1024
}

pub(crate) fn default_tcp_session_idle_timeout_ms() -> u64 {
    0
}

pub(crate) fn default_tcp_max_connection_age_ms() -> u64 {
    0
}

pub(crate) fn default_retry_budget_enabled() -> bool {
    true
}

pub(crate) fn default_retry_budget_ratio_percent() -> u32 {
    20
}

pub(crate) fn default_retry_budget_burst() -> u32 {
    16
}

pub(crate) fn default_graceful_drain_period_ms() -> u64 {
    30_000
}

pub(crate) fn default_active_health_check_enabled() -> bool {
    false
}

pub(crate) fn default_active_health_check_interval_ms() -> u64 {
    5_000
}

pub(crate) fn default_active_health_check_timeout_ms() -> u64 {
    1_000
}

pub(crate) fn default_active_health_check_unhealthy_threshold() -> u32 {
    2
}

pub(crate) fn default_tcp_keepalive_idle_ms() -> u64 {
    60_000
}

pub(crate) fn default_tcp_keepalive_interval_ms() -> u64 {
    15_000
}

pub(crate) fn default_tcp_keepalive_probe_count() -> usize {
    4
}

pub(crate) fn default_xds_connect_timeout_ms() -> u64 {
    5_000
}

pub(crate) fn default_xds_keepalive_interval_ms() -> u64 {
    10_000
}

pub(crate) fn default_xds_keepalive_timeout_ms() -> u64 {
    5_000
}

pub(crate) fn default_xds_initial_reconnect_backoff_ms() -> u64 {
    2_000
}

pub(crate) fn default_xds_max_reconnect_backoff_ms() -> u64 {
    30_000
}

pub(crate) fn default_xds_apply_timeout_ms() -> u64 {
    3_000
}

pub(crate) fn default_xds_apply_poll_interval_ms() -> u64 {
    100
}

pub(crate) fn default_xds_stale_stream_timeout_ms() -> u64 {
    30_000
}

pub(crate) fn default_xds_snapshot_freshness_timeout_ms() -> u64 {
    90_000
}

pub(crate) fn default_http_max_request_body_bytes() -> usize {
    10 * 1024 * 1024
}

pub(crate) fn default_http_max_request_header_bytes() -> usize {
    65_536
}

pub(crate) fn default_ai_gateway_max_request_body_bytes() -> usize {
    10 * 1024 * 1024
}

pub(crate) fn default_work_stealing() -> bool {
    true
}

pub(crate) fn default_upstream_tcp_fast_open() -> bool {
    true
}

pub(crate) fn default_upstream_connection_timeout_ms() -> u64 {
    5_000
}

pub(crate) fn default_upstream_read_timeout_ms() -> u64 {
    15_000
}

pub(crate) fn default_upstream_idle_timeout_ms() -> u64 {
    60_000
}

pub(crate) fn default_stream_upstream_pool_size() -> usize {
    256
}

pub(crate) fn default_stream_upstream_pool_idle_timeout_ms() -> u64 {
    30_000
}

pub(crate) fn millis_duration(value_ms: u64, fallback_ms: u64) -> Duration {
    Duration::from_millis(if value_ms == 0 {
        fallback_ms.max(1)
    } else {
        value_ms
    })
}

pub(crate) fn default_xds_tls_enabled() -> bool {
    true
}

pub(crate) fn optional_millis_duration(value_ms: u64, fallback_ms: u64) -> Option<Duration> {
    let effective = if value_ms == 0 { fallback_ms } else { value_ms };
    (effective > 0).then(|| Duration::from_millis(effective))
}

pub(crate) fn default_http_cache_enabled() -> bool {
    false
}

pub(crate) fn default_http_cache_max_size_mb() -> usize {
    256
}

pub(crate) fn default_http_cache_max_entry_size_mb() -> usize {
    16
}

pub(crate) fn default_http_cache_default_ttl_seconds() -> u64 {
    60
}

#[allow(dead_code)]
pub(crate) fn default_route_max_request_body_bytes() -> usize {
    10 * 1024 * 1024
}

#[allow(dead_code)]
pub(crate) fn default_route_request_body_buffer_bytes() -> usize {
    128 * 1024
}

#[allow(dead_code)]
pub(crate) fn default_route_max_request_header_bytes() -> usize {
    65_536
}

#[allow(dead_code)]
pub(crate) fn default_route_upstream_connect_timeout_ms() -> u64 {
    5_000
}

#[allow(dead_code)]
pub(crate) fn default_route_upstream_read_timeout_ms() -> u64 {
    15_000
}

pub(crate) fn trimmed_non_empty(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}
