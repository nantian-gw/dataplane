use super::{
    AccessLogConfig, ExperimentalConfig, HttpCacheConfig, HttpCapacityConfig, LogConfig,
    OpenTelemetryConfig, RuntimeConfig, RuntimeProtectionConfig, RuntimeTuningConfig, SentryConfig,
    TcpKeepaliveConfig, XdsTransportConfig,
    defaults::{
        default_access_enabled, default_access_format, default_access_mode, default_access_path,
        default_access_sample_rate, default_active_health_check_enabled,
        default_active_health_check_interval_ms, default_active_health_check_timeout_ms,
        default_ai_gateway_max_request_body_bytes, default_active_health_check_unhealthy_threshold, default_wasm_max_concurrency,
        default_downstream_read_timeout_ms, default_graceful_drain_period_ms,
        default_http_cache_default_ttl_seconds, default_http_cache_enabled,
        default_http_cache_max_entry_size_mb, default_http_cache_max_size_mb,
        default_http_keepalive_request_limit, default_http_listen_addr,
        default_http_max_connection_age_ms, default_http_max_request_body_bytes,
        default_http_max_request_header_bytes, default_http_reload_retry_interval_ms,
        default_http3_enabled, default_level, default_log_drop_when_full, default_log_format,
        default_log_non_blocking, default_log_non_blocking_buffered_lines,
        default_open_telemetry_protocol, default_open_telemetry_sample_ratio,
        default_open_telemetry_service_name, default_open_telemetry_timeout_ms,
        default_request_mirror_max_concurrency, default_retry_budget_burst,
        default_retry_budget_enabled, default_retry_budget_ratio_percent,
        default_route_annotation_prefix, default_sentry_sample_rate,
        default_sentry_traces_sample_rate, default_stream_reload_retry_interval_ms,
        default_stream_upstream_pool_idle_timeout_ms, default_stream_upstream_pool_size,
        default_tcp_keepalive_idle_ms, default_tcp_keepalive_interval_ms,
        default_tcp_keepalive_probe_count, default_tcp_max_connection_age_ms,
        default_tcp_proxy_buffer_bytes, default_tcp_session_idle_timeout_ms, default_tls_max,
        default_tls_min, default_true, default_udp_response_idle_timeout_ms,
        default_upstream_connection_timeout_ms, default_upstream_idle_timeout_ms,
        default_upstream_read_timeout_ms, default_upstream_tcp_fast_open, default_work_stealing,
        default_xds_apply_poll_interval_ms, default_xds_apply_timeout_ms,
        default_xds_connect_timeout_ms, default_xds_initial_reconnect_backoff_ms,
        default_xds_keepalive_interval_ms, default_xds_keepalive_timeout_ms,
        default_xds_max_reconnect_backoff_ms, default_xds_snapshot_freshness_timeout_ms,
        default_xds_stale_stream_timeout_ms,
    },
};
use std::collections::BTreeMap;

impl Default for LogConfig {
    fn default() -> Self {
        Self {
            level: default_level(),
            format: default_log_format(),
            add_source: false,
            include_target: false,
            include_thread_ids: false,
            include_thread_names: false,
            non_blocking: default_log_non_blocking(),
            non_blocking_buffered_lines: default_log_non_blocking_buffered_lines(),
            drop_when_full: default_log_drop_when_full(),
            open_telemetry: OpenTelemetryConfig::default(),
            sentry: SentryConfig::default(),
        }
    }
}

impl Default for SentryConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            dsn: String::new(),
            environment: String::new(),
            sample_rate: default_sentry_sample_rate(),
            traces_sample_rate: default_sentry_traces_sample_rate(),
            attach_stacktrace: default_true(),
            send_default_pii: false,
            debug: false,
        }
    }
}

impl Default for OpenTelemetryConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            endpoint: String::new(),
            protocol: default_open_telemetry_protocol(),
            timeout_ms: default_open_telemetry_timeout_ms(),
            insecure: false,
            sample_ratio: default_open_telemetry_sample_ratio(),
            service_name: default_open_telemetry_service_name(),
            service_namespace: String::new(),
        }
    }
}

impl Default for AccessLogConfig {
    fn default() -> Self {
        Self {
            enabled: default_access_enabled(),
            path: default_access_path(),
            format: default_access_format(),
            mode: default_access_mode(),
            formats: BTreeMap::new(),
            format_name: String::new(),
            sample_rate: default_access_sample_rate(),
            route_annotation_prefix: default_route_annotation_prefix(),
        }
    }
}

impl Default for RuntimeConfig {
    fn default() -> Self {
        Self {
            http_listen_addr: default_http_listen_addr(),
            enable_ipv6: true,
            enable_http3: default_http3_enabled(),
            tls_min_version: default_tls_min(),
            tls_max_version: default_tls_max(),
            tls_asset_dir: String::new(),
        }
    }
}

impl Default for XdsTransportConfig {
    fn default() -> Self {
        Self {
            connect_timeout_ms: default_xds_connect_timeout_ms(),
            keepalive_interval_ms: default_xds_keepalive_interval_ms(),
            keepalive_timeout_ms: default_xds_keepalive_timeout_ms(),
            initial_reconnect_backoff_ms: default_xds_initial_reconnect_backoff_ms(),
            max_reconnect_backoff_ms: default_xds_max_reconnect_backoff_ms(),
            apply_timeout_ms: default_xds_apply_timeout_ms(),
            apply_poll_interval_ms: default_xds_apply_poll_interval_ms(),
            stale_stream_timeout_ms: default_xds_stale_stream_timeout_ms(),
            snapshot_freshness_timeout_ms: default_xds_snapshot_freshness_timeout_ms(),
        }
    }
}

impl Default for RuntimeTuningConfig {
    fn default() -> Self {
        Self {
            http_reload_retry_interval_ms: default_http_reload_retry_interval_ms(),
            stream_reload_retry_interval_ms: default_stream_reload_retry_interval_ms(),
            downstream_read_timeout_ms: default_downstream_read_timeout_ms(),
            http_max_connection_age_ms: default_http_max_connection_age_ms(),
            http_keepalive_request_limit: default_http_keepalive_request_limit(),
            http_capacity: HttpCapacityConfig {
                upstream_keepalive_pool_size: 1024,
                ..Default::default()
            },
            request_mirror_max_concurrency: default_request_mirror_max_concurrency(),
            udp_response_idle_timeout_ms: default_udp_response_idle_timeout_ms(),
            tcp_proxy_buffer_bytes: default_tcp_proxy_buffer_bytes(),
            tcp_session_idle_timeout_ms: default_tcp_session_idle_timeout_ms(),
            tcp_max_connection_age_ms: default_tcp_max_connection_age_ms(),
            stream_upstream_pool_size: default_stream_upstream_pool_size(),
            stream_upstream_pool_idle_timeout_ms: default_stream_upstream_pool_idle_timeout_ms(),
            retry_budget_enabled: default_retry_budget_enabled(),
            retry_budget_ratio_percent: default_retry_budget_ratio_percent(),
            retry_budget_burst: default_retry_budget_burst(),
            graceful_drain_period_ms: default_graceful_drain_period_ms(),
            active_health_check_enabled: default_active_health_check_enabled(),
            active_health_check_interval_ms: default_active_health_check_interval_ms(),
            active_health_check_timeout_ms: default_active_health_check_timeout_ms(),
            active_health_check_unhealthy_threshold:
                default_active_health_check_unhealthy_threshold(),
            downstream_tcp_keepalive: TcpKeepaliveConfig::default(),
            upstream_tcp_keepalive: TcpKeepaliveConfig::default(),
            work_stealing: default_work_stealing(),
            downstream_tcp_fastopen: None,
            downstream_dscp: None,
            upstream_tcp_recv_buf: 0,
            upstream_tcp_fast_open: default_upstream_tcp_fast_open(),
            upstream_connection_timeout_ms: default_upstream_connection_timeout_ms(),
            upstream_read_timeout_ms: default_upstream_read_timeout_ms(),
            upstream_idle_timeout_ms: default_upstream_idle_timeout_ms(),
            upstream_dscp: None,
            http_cache: HttpCacheConfig::default(),
        }
    }
}

impl Default for HttpCacheConfig {
    fn default() -> Self {
        Self {
            enabled: default_http_cache_enabled(),
            max_size_mb: default_http_cache_max_size_mb(),
            max_entry_size_mb: default_http_cache_max_entry_size_mb(),
            default_ttl_seconds: default_http_cache_default_ttl_seconds(),
        }
    }
}

impl Default for ExperimentalConfig {
    fn default() -> Self {
        Self {
            enable_experimental_gateway: false,
            enable_ai_gateway: false,
            ai_gateway_max_request_body_bytes: default_ai_gateway_max_request_body_bytes(),
            wasm_max_concurrency: default_wasm_max_concurrency(),
        }
    }
}

impl Default for TcpKeepaliveConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            idle_ms: default_tcp_keepalive_idle_ms(),
            interval_ms: default_tcp_keepalive_interval_ms(),
            probe_count: default_tcp_keepalive_probe_count(),
            user_timeout_ms: 0,
        }
    }
}

impl Default for RuntimeProtectionConfig {
    fn default() -> Self {
        Self {
            http_global_inflight_limit: 0,
            http_listener_inflight_limit: 0,
            http_route_inflight_limit: 0,
            http_backend_circuit_breaker_max_requests: 0,
            http_global_rate_limit_requests_per_second: 0,
            http_global_rate_limit_burst: 0,
            http_listener_rate_limit_requests_per_second: 0,
            http_listener_rate_limit_burst: 0,
            http_route_rate_limit_requests_per_second: 0,
            http_route_rate_limit_burst: 0,
            http_max_request_body_bytes: default_http_max_request_body_bytes(),
            http_max_request_header_bytes: default_http_max_request_header_bytes(),
            tcp_global_connection_limit: 0,
            tcp_listener_connection_limit: 0,
            udp_global_datagram_limit: 0,
            udp_listener_datagram_limit: 0,
        }
    }
}
