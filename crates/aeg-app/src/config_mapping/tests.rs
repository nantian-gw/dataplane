use super::{
    to_http_runtime_options, to_stream_runtime_options, to_tracing_options, to_xds_runtime_config,
};
use aeg_config::{
    AccessLogConfig, AdminAuthConfig, DataPlaneConfig, ExperimentalConfig, HttpCapacityConfig,
    LogConfig, RuntimeConfig, RuntimeProtectionConfig, RuntimeTuningConfig,
    SessionPersistenceConfig, XdsTlsConfig, XdsTransportConfig,
};

include!("tests/runtime_tuning.rs");
include!("tests/tracing.rs");
include!("tests/xds_transport.rs");

fn base_config() -> DataPlaneConfig {
    DataPlaneConfig {
        node_id: "dp".to_string(),
        cluster: "kind".to_string(),
        control_plane_addr: "http://127.0.0.1:18080".to_string(),
        admin_addr: "127.0.0.1:19080".to_string(),
        log: LogConfig::default(),
        access_log: AccessLogConfig::default(),
        admin_auth: AdminAuthConfig::default(),
        runtime: RuntimeConfig::default(),
        session_persistence: SessionPersistenceConfig::default(),
        xds_tls: XdsTlsConfig::default(),
        xds_transport: XdsTransportConfig::default(),
        runtime_protection: RuntimeProtectionConfig::default(),
        runtime_tuning: RuntimeTuningConfig::default(),
        experimental: ExperimentalConfig::default(),
    }
}
