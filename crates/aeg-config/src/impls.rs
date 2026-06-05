use std::{fs, path::Path, time::Duration};

use anyhow::Result;
use pingora::protocols::l4::ext::TcpKeepalive;
use sha2::{Digest, Sha256};

use super::{
    defaults::{
        default_active_health_check_interval_ms, default_active_health_check_timeout_ms,
        default_downstream_read_timeout_ms, default_http_max_connection_age_ms,
        default_http_reload_retry_interval_ms, default_stream_reload_retry_interval_ms,
        default_tcp_keepalive_idle_ms, default_tcp_keepalive_interval_ms,
        default_tcp_max_connection_age_ms, default_tcp_proxy_buffer_bytes,
        default_tcp_session_idle_timeout_ms, default_udp_response_idle_timeout_ms,
        default_xds_apply_poll_interval_ms, default_xds_apply_timeout_ms,
        default_xds_connect_timeout_ms, default_xds_initial_reconnect_backoff_ms,
        default_xds_keepalive_interval_ms, default_xds_keepalive_timeout_ms,
        default_xds_max_reconnect_backoff_ms, default_xds_snapshot_freshness_timeout_ms,
        default_xds_stale_stream_timeout_ms, millis_duration, optional_millis_duration,
        trimmed_non_empty,
    },
    AdminAuthConfig, DataPlaneConfig, HttpCapacityConfig, RuntimeTuningConfig,
    SessionPersistenceConfig, TcpKeepaliveConfig, XdsTlsConfig, XdsTransportConfig,
};

impl DataPlaneConfig {
    pub fn load(path: impl AsRef<Path>) -> Result<Self> {
        let raw = fs::read_to_string(path)?;
        let mut cfg: Self = serde_yaml::from_str(&raw)?;
        cfg.apply_env_overrides(|key| std::env::var(key).ok());
        Ok(cfg)
    }

    pub(crate) fn apply_env_overrides<F>(&mut self, lookup: F)
    where
        F: Fn(&str) -> Option<String>,
    {
        if let Some(value) = lookup("AEG_NODE_ID").and_then(|value| trimmed_non_empty(&value)) {
            self.node_id = value;
        }
        if let Some(value) = lookup("AEG_CLUSTER").and_then(|value| trimmed_non_empty(&value)) {
            self.cluster = value;
        }
        if let Some(value) =
            lookup("AEG_CONTROL_PLANE_ADDR").and_then(|value| trimmed_non_empty(&value))
        {
            self.control_plane_addr = value;
        }
        if let Some(value) = lookup("AEG_ADMIN_ADDR").and_then(|value| trimmed_non_empty(&value)) {
            self.admin_addr = value;
        }
    }
}

impl AdminAuthConfig {
    pub fn resolve_bearer_token(&self) -> Result<Option<String>> {
        if let Some(path) = trimmed_non_empty(&self.bearer_token_file) {
            let raw = fs::read_to_string(path)?;
            return Ok(trimmed_non_empty(&raw));
        }

        Ok(trimmed_non_empty(&self.bearer_token))
    }
}

impl XdsTlsConfig {
    pub fn enabled(&self) -> bool {
        self.enabled
            || !self.ca_path.trim().is_empty()
            || (!self.cert_path.trim().is_empty() && !self.key_path.trim().is_empty())
    }
}

impl XdsTransportConfig {
    pub fn connect_timeout(&self) -> Duration {
        millis_duration(self.connect_timeout_ms, default_xds_connect_timeout_ms())
    }

    pub fn keepalive_interval(&self) -> Duration {
        millis_duration(
            self.keepalive_interval_ms,
            default_xds_keepalive_interval_ms(),
        )
    }

    pub fn keepalive_timeout(&self) -> Duration {
        millis_duration(
            self.keepalive_timeout_ms,
            default_xds_keepalive_timeout_ms(),
        )
    }

    pub fn initial_reconnect_backoff(&self) -> Duration {
        millis_duration(
            self.initial_reconnect_backoff_ms,
            default_xds_initial_reconnect_backoff_ms(),
        )
    }

    pub fn max_reconnect_backoff(&self) -> Duration {
        let max_ms = self
            .max_reconnect_backoff_ms
            .max(self.initial_reconnect_backoff_ms.max(1));
        millis_duration(max_ms, default_xds_max_reconnect_backoff_ms())
    }

    pub fn apply_timeout(&self) -> Duration {
        millis_duration(self.apply_timeout_ms, default_xds_apply_timeout_ms())
    }

    pub fn apply_poll_interval(&self) -> Duration {
        millis_duration(
            self.apply_poll_interval_ms,
            default_xds_apply_poll_interval_ms(),
        )
    }

    pub fn stale_stream_timeout(&self) -> Duration {
        millis_duration(
            self.stale_stream_timeout_ms,
            default_xds_stale_stream_timeout_ms(),
        )
    }

    pub fn snapshot_freshness_timeout(&self) -> Duration {
        millis_duration(
            self.snapshot_freshness_timeout_ms,
            default_xds_snapshot_freshness_timeout_ms(),
        )
    }
}

impl RuntimeTuningConfig {
    pub fn http_reload_retry_interval(&self) -> Duration {
        millis_duration(
            self.http_reload_retry_interval_ms,
            default_http_reload_retry_interval_ms(),
        )
    }

    pub fn stream_reload_retry_interval(&self) -> Duration {
        millis_duration(
            self.stream_reload_retry_interval_ms,
            default_stream_reload_retry_interval_ms(),
        )
    }

    pub fn downstream_read_timeout(&self) -> Option<Duration> {
        optional_millis_duration(
            self.downstream_read_timeout_ms,
            default_downstream_read_timeout_ms(),
        )
    }

    pub fn http_keepalive_request_limit(&self) -> Option<u32> {
        if self.http_keepalive_request_limit == 0 {
            None
        } else {
            Some(self.http_keepalive_request_limit.max(1))
        }
    }

    pub fn http_capacity(&self) -> HttpCapacityConfig {
        self.http_capacity.clone()
    }

    pub fn http_max_connection_age(&self) -> Option<Duration> {
        optional_millis_duration(
            self.http_max_connection_age_ms,
            default_http_max_connection_age_ms(),
        )
    }

    pub fn udp_response_idle_timeout(&self) -> Duration {
        millis_duration(
            self.udp_response_idle_timeout_ms,
            default_udp_response_idle_timeout_ms(),
        )
    }

    pub fn tcp_proxy_buffer_bytes(&self) -> usize {
        if self.tcp_proxy_buffer_bytes == 0 {
            default_tcp_proxy_buffer_bytes()
        } else {
            self.tcp_proxy_buffer_bytes
        }
    }

    pub fn tcp_session_idle_timeout(&self) -> Option<Duration> {
        optional_millis_duration(
            self.tcp_session_idle_timeout_ms,
            default_tcp_session_idle_timeout_ms(),
        )
    }

    pub fn tcp_max_connection_age(&self) -> Option<Duration> {
        optional_millis_duration(
            self.tcp_max_connection_age_ms,
            default_tcp_max_connection_age_ms(),
        )
    }

    pub fn stream_upstream_pool_size(&self) -> usize {
        self.stream_upstream_pool_size
    }

    pub fn stream_upstream_pool_idle_timeout(&self) -> Duration {
        Duration::from_millis(self.stream_upstream_pool_idle_timeout_ms)
    }

    pub fn retry_budget_enabled(&self) -> bool {
        self.retry_budget_enabled
    }

    pub fn retry_budget_ratio_percent(&self) -> u32 {
        self.retry_budget_ratio_percent.min(100)
    }

    pub fn retry_budget_burst(&self) -> u32 {
        self.retry_budget_burst.max(1)
    }

    pub fn graceful_drain_period(&self) -> Duration {
        Duration::from_millis(self.graceful_drain_period_ms)
    }

    pub fn active_health_check_enabled(&self) -> bool {
        self.active_health_check_enabled
    }

    pub fn active_health_check_interval(&self) -> Duration {
        millis_duration(
            self.active_health_check_interval_ms,
            default_active_health_check_interval_ms(),
        )
    }

    pub fn active_health_check_timeout(&self) -> Duration {
        millis_duration(
            self.active_health_check_timeout_ms,
            default_active_health_check_timeout_ms(),
        )
    }

    pub fn active_health_check_unhealthy_threshold(&self) -> u32 {
        self.active_health_check_unhealthy_threshold.max(1)
    }

    pub fn downstream_tcp_keepalive(&self) -> Option<TcpKeepalive> {
        self.downstream_tcp_keepalive.to_nantian()
    }

    pub fn upstream_tcp_keepalive(&self) -> Option<TcpKeepalive> {
        self.upstream_tcp_keepalive.to_nantian()
    }

    pub fn work_stealing(&self) -> bool {
        self.work_stealing
    }

    pub fn downstream_tcp_fastopen(&self) -> Option<usize> {
        self.downstream_tcp_fastopen
    }

    pub fn downstream_dscp(&self) -> Option<u8> {
        self.downstream_dscp
    }

    pub fn upstream_tcp_recv_buf(&self) -> Option<usize> {
        (self.upstream_tcp_recv_buf > 0).then_some(self.upstream_tcp_recv_buf)
    }

    pub fn upstream_tcp_fast_open(&self) -> bool {
        self.upstream_tcp_fast_open
    }

    pub fn upstream_connection_timeout(&self) -> Option<Duration> {
        optional_millis_duration(self.upstream_connection_timeout_ms, 0)
    }

    pub fn upstream_read_timeout(&self) -> Option<Duration> {
        optional_millis_duration(self.upstream_read_timeout_ms, 0)
    }

    pub fn upstream_idle_timeout(&self) -> Option<Duration> {
        optional_millis_duration(self.upstream_idle_timeout_ms, 0)
    }

    pub fn upstream_dscp(&self) -> Option<u8> {
        self.upstream_dscp
    }
}

impl TcpKeepaliveConfig {
    pub fn to_nantian(&self) -> Option<TcpKeepalive> {
        if !self.enabled {
            return None;
        }

        Some(TcpKeepalive {
            idle: millis_duration(self.idle_ms, default_tcp_keepalive_idle_ms()),
            interval: millis_duration(self.interval_ms, default_tcp_keepalive_interval_ms()),
            count: self.probe_count.max(1),
            #[cfg(target_os = "linux")]
            user_timeout: Duration::from_millis(self.user_timeout_ms),
        })
    }
}

impl SessionPersistenceConfig {
    pub fn resolve_shared_secret(&self) -> Option<Vec<u8>> {
        let secret = trimmed_non_empty(self.shared_secret.as_deref().unwrap_or(""))?;
        let hash = Sha256::digest(secret.as_bytes());
        Some(hash.to_vec())
    }

    pub fn resolve_secret(&self) -> Result<Option<Vec<u8>>> {
        if let Some(path) = trimmed_non_empty(&self.secret_key_file) {
            let raw = fs::read_to_string(path)?;
            return Ok(trimmed_non_empty(&raw).map(|secret| secret.into_bytes()));
        }

        Ok(trimmed_non_empty(&self.secret_key).map(|secret| secret.into_bytes()))
    }
}
