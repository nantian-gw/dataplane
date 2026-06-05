use aeg_config::{AccessLogConfig, DataPlaneConfig, HttpCapacityConfig, RuntimeProtectionConfig};
use aeg_observability::{
    AccessLogMode, HttpAdmissionOptions, HttpCircuitBreakerOptions, HttpRateLimitOptions,
    OpenTelemetryOptions, RetryBudgetOptions, TcpAdmissionOptions, TracingOptions,
    UdpAdmissionOptions,
};
use aeg_xds::{ClientTlsOptions, ConnectOptions, TransportOptions};

use crate::xds_runtime::XdsRuntimeConfig;

pub(crate) fn to_tracing_options(cfg: &DataPlaneConfig) -> TracingOptions {
    TracingOptions {
        level: cfg.log.level.clone(),
        format: cfg.log.format.clone(),
        add_source: cfg.log.add_source,
        include_target: cfg.log.include_target,
        include_thread_ids: cfg.log.include_thread_ids,
        include_thread_names: cfg.log.include_thread_names,
        non_blocking: cfg.log.non_blocking,
        non_blocking_buffered_lines: cfg.log.non_blocking_buffered_lines,
        drop_when_full: cfg.log.drop_when_full,
        open_telemetry: OpenTelemetryOptions {
            enabled: cfg.log.open_telemetry.enabled,
            endpoint: cfg.log.open_telemetry.endpoint.clone(),
            protocol: cfg.log.open_telemetry.protocol.clone(),
            timeout_ms: cfg.log.open_telemetry.timeout_ms,
            insecure: cfg.log.open_telemetry.insecure,
            sample_ratio: cfg.log.open_telemetry.sample_ratio,
            service_name: cfg.log.open_telemetry.service_name.clone(),
            service_namespace: cfg.log.open_telemetry.service_namespace.clone(),
            service_instance_id: cfg.node_id.clone(),
            deployment_environment: cfg.cluster.clone(),
        },
    }
}

pub(crate) fn to_access_log_options(cfg: &AccessLogConfig) -> aeg_observability::AccessLogOptions {
    aeg_observability::AccessLogOptions {
        enabled: cfg.enabled,
        path: cfg.path.clone(),
        format: cfg.format.clone(),
        mode: AccessLogMode::parse(&cfg.mode),
        sample_rate: cfg.sample_rate.clamp(0.0, 1.0),
        route_annotation_prefix: cfg.route_annotation_prefix.clone(),
    }
}

pub(crate) fn to_http_admission_options(cfg: &RuntimeProtectionConfig) -> HttpAdmissionOptions {
    HttpAdmissionOptions {
        global_inflight_limit: cfg.http_global_inflight_limit,
        listener_inflight_limit: cfg.http_listener_inflight_limit,
        route_inflight_limit: cfg.http_route_inflight_limit,
    }
}

pub(crate) fn to_tcp_admission_options(cfg: &RuntimeProtectionConfig) -> TcpAdmissionOptions {
    TcpAdmissionOptions {
        global_connection_limit: cfg.tcp_global_connection_limit,
        listener_connection_limit: cfg.tcp_listener_connection_limit,
    }
}

pub(crate) fn to_http_rate_limit_options(cfg: &RuntimeProtectionConfig) -> HttpRateLimitOptions {
    HttpRateLimitOptions {
        global_requests_per_second: cfg.http_global_rate_limit_requests_per_second,
        global_burst: if cfg.http_global_rate_limit_requests_per_second > 0 {
            cfg.http_global_rate_limit_burst.max(1)
        } else {
            0
        },
        listener_requests_per_second: cfg.http_listener_rate_limit_requests_per_second,
        listener_burst: if cfg.http_listener_rate_limit_requests_per_second > 0 {
            cfg.http_listener_rate_limit_burst.max(1)
        } else {
            0
        },
        route_requests_per_second: cfg.http_route_rate_limit_requests_per_second,
        route_burst: if cfg.http_route_rate_limit_requests_per_second > 0 {
            cfg.http_route_rate_limit_burst.max(1)
        } else {
            0
        },
    }
}

pub(crate) fn to_http_circuit_breaker_options(
    cfg: &RuntimeProtectionConfig,
) -> HttpCircuitBreakerOptions {
    HttpCircuitBreakerOptions {
        backend_max_inflight_requests: cfg.http_backend_circuit_breaker_max_requests,
    }
}

pub(crate) fn to_udp_admission_options(cfg: &RuntimeProtectionConfig) -> UdpAdmissionOptions {
    UdpAdmissionOptions {
        global_datagram_limit: cfg.udp_global_datagram_limit,
        listener_datagram_limit: cfg.udp_listener_datagram_limit,
    }
}

pub(crate) fn to_http_capacity_options(cfg: &HttpCapacityConfig) -> aeg_http::HttpCapacityOptions {
    aeg_http::HttpCapacityOptions {
        worker_threads: cfg.worker_threads,
        accept_concurrency: cfg.accept_concurrency,
        upstream_keepalive_pool_size: cfg.upstream_keepalive_pool_size,
        reuse_port: cfg.reuse_port,
    }
}

pub(crate) fn to_http_runtime_options(cfg: &DataPlaneConfig) -> aeg_http::RuntimeOptions {
    let http_capacity = cfg.runtime_tuning.http_capacity();

    aeg_http::RuntimeOptions {
        default_listen_addr: cfg.runtime.http_listen_addr.clone(),
        enable_ipv6: cfg.runtime.enable_ipv6,
        enable_http3: cfg.runtime.enable_http3,
        tls_min_version: cfg.runtime.tls_min_version.clone(),
        tls_max_version: cfg.runtime.tls_max_version.clone(),
        tls_asset_dir: cfg.runtime.tls_asset_dir.clone(),
        reload_retry_interval: cfg.runtime_tuning.http_reload_retry_interval(),
        downstream_read_timeout: cfg.runtime_tuning.downstream_read_timeout(),
        downstream_max_connection_age: cfg.runtime_tuning.http_max_connection_age(),
        keepalive_request_limit: cfg.runtime_tuning.http_keepalive_request_limit(),
        capacity: to_http_capacity_options(&http_capacity),
        downstream_tcp_keepalive: cfg.runtime_tuning.downstream_tcp_keepalive(),
        upstream_tcp_keepalive: cfg.runtime_tuning.upstream_tcp_keepalive(),
        request_tracing_enabled: cfg.log.open_telemetry.enabled,
        admission: to_http_admission_options(&cfg.runtime_protection),
        circuit_breaker: to_http_circuit_breaker_options(&cfg.runtime_protection),
        rate_limit: to_http_rate_limit_options(&cfg.runtime_protection),
        retry_budget: RetryBudgetOptions {
            enabled: cfg.runtime_tuning.retry_budget_enabled(),
            ratio_percent: cfg.runtime_tuning.retry_budget_ratio_percent(),
            burst: cfg.runtime_tuning.retry_budget_burst(),
        },
        max_request_body_bytes: cfg.runtime_protection.http_max_request_body_bytes,
        max_request_header_bytes: cfg.runtime_protection.http_max_request_header_bytes,
        work_stealing: cfg.runtime_tuning.work_stealing(),
        downstream_tcp_fastopen: cfg.runtime_tuning.downstream_tcp_fastopen(),
        downstream_dscp: cfg.runtime_tuning.downstream_dscp(),
        upstream_tcp_fast_open: cfg.runtime_tuning.upstream_tcp_fast_open(),
        upstream_tcp_recv_buf: cfg.runtime_tuning.upstream_tcp_recv_buf(),
        upstream_connection_timeout: cfg.runtime_tuning.upstream_connection_timeout(),
        upstream_read_timeout: cfg.runtime_tuning.upstream_read_timeout(),
        upstream_idle_timeout: cfg.runtime_tuning.upstream_idle_timeout(),
        upstream_dscp: cfg.runtime_tuning.upstream_dscp(),
        cache: aeg_http::CacheManager::new(aeg_http::CacheOptions::from(
            &cfg.runtime_tuning.http_cache,
        )),
        experimental: cfg.experimental.clone(),
    }
}

pub(crate) fn to_stream_runtime_options(cfg: &DataPlaneConfig) -> aeg_stream::RuntimeOptions {
    aeg_stream::RuntimeOptions {
        reload_retry_interval: cfg.runtime_tuning.stream_reload_retry_interval(),
        udp_response_idle_timeout: cfg.runtime_tuning.udp_response_idle_timeout(),
        tcp_proxy_buffer_bytes: cfg.runtime_tuning.tcp_proxy_buffer_bytes(),
        tcp_session_idle_timeout: cfg.runtime_tuning.tcp_session_idle_timeout(),
        tcp_max_connection_age: cfg.runtime_tuning.tcp_max_connection_age(),
        tcp_admission: to_tcp_admission_options(&cfg.runtime_protection),
        udp_admission: to_udp_admission_options(&cfg.runtime_protection),
        stream_upstream_pool_size: cfg.runtime_tuning.stream_upstream_pool_size(),
        stream_upstream_pool_idle_timeout: cfg.runtime_tuning.stream_upstream_pool_idle_timeout(),
    }
}

pub(crate) fn to_xds_runtime_config(cfg: &DataPlaneConfig) -> XdsRuntimeConfig {
    XdsRuntimeConfig {
        connect_options: ConnectOptions {
            endpoint: cfg.control_plane_addr.clone(),
            tls: cfg.xds_tls.enabled().then(|| ClientTlsOptions {
                ca_path: cfg.xds_tls.ca_path.clone(),
                cert_path: cfg.xds_tls.cert_path.clone(),
                key_path: cfg.xds_tls.key_path.clone(),
                domain_name: cfg.xds_tls.domain_name.clone(),
            }),
            transport: TransportOptions {
                connect_timeout: cfg.xds_transport.connect_timeout(),
                keepalive_interval: cfg.xds_transport.keepalive_interval(),
                keepalive_timeout: cfg.xds_transport.keepalive_timeout(),
                initial_reconnect_backoff: cfg.xds_transport.initial_reconnect_backoff(),
                max_reconnect_backoff: cfg.xds_transport.max_reconnect_backoff(),
                apply_timeout: cfg.xds_transport.apply_timeout(),
                apply_poll_interval: cfg.xds_transport.apply_poll_interval(),
                stale_stream_timeout: cfg.xds_transport.stale_stream_timeout(),
            },
        },
        node_id: cfg.node_id.clone(),
        cluster: cfg.cluster.clone(),
    }
}

#[cfg(test)]
mod tests;
