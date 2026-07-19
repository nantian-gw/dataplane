use std::time::Duration;

use pingora::protocols::l4::ext::TcpKeepalive;

use ntgw_observability::{
    AccessLogOptions, HttpAdmissionOptions, HttpCircuitBreakerOptions, HttpRateLimitOptions,
    RetryBudgetOptions,
};

use crate::session::SessionPersistenceOptions;

use super::capacity::HttpCapacityOptions;

#[derive(Debug, Clone)]
pub struct RuntimeOptions {
    pub default_listen_addr: String,
    pub enable_ipv6: bool,
    pub enable_http3: bool,
    pub tls_min_version: String,
    pub tls_max_version: String,
    pub tls_asset_dir: String,
    pub reload_retry_interval: Duration,
    pub downstream_read_timeout: Option<Duration>,
    pub downstream_max_connection_age: Option<Duration>,
    pub keepalive_request_limit: Option<u32>,
    pub capacity: HttpCapacityOptions,
    pub downstream_tcp_keepalive: Option<TcpKeepalive>,
    pub upstream_tcp_keepalive: Option<TcpKeepalive>,
    pub request_tracing_enabled: bool,
    pub admission: HttpAdmissionOptions,
    pub circuit_breaker: HttpCircuitBreakerOptions,
    pub rate_limit: HttpRateLimitOptions,
    pub retry_budget: RetryBudgetOptions,
    pub max_request_body_bytes: usize,
    pub max_request_header_bytes: usize,
    pub work_stealing: bool,
    pub downstream_tcp_fastopen: Option<usize>,
    pub downstream_dscp: Option<u8>,
    pub upstream_tcp_fast_open: bool,
    pub upstream_tcp_recv_buf: Option<usize>,
    pub upstream_connection_timeout: Option<Duration>,
    pub upstream_read_timeout: Option<Duration>,
    pub upstream_idle_timeout: Option<Duration>,
    pub upstream_dscp: Option<u8>,
    pub cache: std::sync::Arc<crate::cache::CacheManager>,
    pub experimental: ntgw_config::ExperimentalConfig,
}

#[derive(Clone)]
pub struct ReloadableRuntimeConfig {
    pub runtime: RuntimeOptions,
    pub access_log: AccessLogOptions,
    pub session_persistence: SessionPersistenceOptions,
}

impl Default for RuntimeOptions {
    fn default() -> Self {
        Self {
            default_listen_addr: String::new(),
            enable_ipv6: true,
            enable_http3: false,
            tls_min_version: "1.2".to_string(),
            tls_max_version: "1.3".to_string(),
            tls_asset_dir: String::new(),
            reload_retry_interval: Duration::from_secs(1),
            downstream_read_timeout: Some(Duration::from_secs(60)),
            downstream_max_connection_age: None,
            keepalive_request_limit: None,
            capacity: HttpCapacityOptions::default(),
            downstream_tcp_keepalive: None,
            upstream_tcp_keepalive: None,
            request_tracing_enabled: false,
            admission: HttpAdmissionOptions::default(),
            circuit_breaker: HttpCircuitBreakerOptions::default(),
            rate_limit: HttpRateLimitOptions::default(),
            retry_budget: RetryBudgetOptions::default(),
            max_request_body_bytes: 0,
            max_request_header_bytes: 0,
            work_stealing: true,
            downstream_tcp_fastopen: None,
            downstream_dscp: None,
            upstream_tcp_fast_open: false,
            upstream_tcp_recv_buf: None,
            upstream_connection_timeout: None,
            upstream_read_timeout: None,
            upstream_idle_timeout: None,
            upstream_dscp: None,
            cache: crate::cache::CacheManager::new(crate::cache::CacheOptions {
                enabled: false,
                max_size_bytes: 0,
                max_entry_size_bytes: 0,
                default_ttl: Duration::from_secs(0),
            }),
            experimental: ntgw_config::ExperimentalConfig::default(),
        }
    }
}
