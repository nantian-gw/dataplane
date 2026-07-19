use std::{
    collections::{BTreeMap, BTreeSet},
    fs, io,
    path::{Path, PathBuf},
    thread,
    time::Duration,
};

use anyhow::{Context, Result, anyhow};
use async_trait::async_trait;
use pingora::{
    apps::HttpServerOptions,
    listeners::tls::TlsSettings,
    protocols::l4::ext::TcpKeepalive,
    proxy::ProxyServiceBuilder,
    server::{RunArgs, Server, ShutdownSignal, ShutdownSignalWatch},
    tls::{pkey::PKey, ssl::SslVerifyMode, x509::X509},
};
use tokio::sync::watch;
use tracing::{error, info, warn};

use ntgw_ir::{
    Listener, SecretMaterial, SharedSnapshot, Snapshot, TlsConfig,
};
use ntgw_observability::{
    AccessLogOptions, HttpAdmissionController, HttpCircuitBreakerController,
    HttpRateLimitController, RetryBudgetController, RuntimeStatsSnapshot, SharedOverloadStats, SharedRuntimeStats, SharedTrafficStats,
};

use crate::proxy::GatewayProxy;
use crate::session::SessionPersistenceOptions;
const LISTENER_ADDRESSES_METADATA_KEY: &str = "nantian.dev/listener-addresses";

pub fn http3_available() -> bool {
    false
}

#[cfg(not(test))]
fn probe_listener_bind(bind: &str) -> io::Result<()> {
    use std::net::TcpListener as StdTcpListener;

    StdTcpListener::bind(bind).map(drop)
}

fn tls_passthrough_binds(snapshot: &Snapshot, runtime: &RuntimeOptions) -> BTreeSet<String> {
    snapshot
        .listeners
        .iter()
        .filter(|listener| {
            listener
                .tls
                .as_ref()
                .is_some_and(|tls| tls.enabled && tls.passthrough)
        })
        .flat_map(|listener| listener_bind_addrs(listener, runtime))
        .collect()
}

fn should_defer_http_listener_bind_handoff(
    snapshot: &Snapshot,
    runtime_state: &RuntimeStatsSnapshot,
    protocol: &ListenerProtocol,
    bind: &str,
    blocked_tls_binds: &BTreeSet<String>,
    err: &io::Error,
) -> bool {
    if !matches!(protocol, ListenerProtocol::Tls(_)) {
        return false;
    }
    if err.kind() != io::ErrorKind::AddrInUse {
        return false;
    }
    if snapshot.id.is_empty() || blocked_tls_binds.contains(bind) {
        return false;
    }

    let has_prior_stream_state = !runtime_state.stream_last_reload_attempt_version.is_empty()
        || !runtime_state.stream_last_good_reload_version.is_empty();
    has_prior_stream_state && runtime_state.stream_last_good_reload_version != snapshot.id
}

fn should_suppress_unavailable_bind_warning(bind: &str, err: &io::Error) -> bool {
    bind.starts_with('[') && is_address_family_not_supported(err)
}

#[cfg(any(target_os = "linux", target_os = "android"))]
fn is_address_family_not_supported(err: &io::Error) -> bool {
    err.raw_os_error() == Some(97)
}

#[cfg(not(any(target_os = "linux", target_os = "android")))]
fn is_address_family_not_supported(_err: &io::Error) -> bool {
    false
}

pub mod bench;
mod capacity;
mod listener_plan;
mod listener_set;
mod options;
mod plan;
mod spawn;
mod updates;
mod server;

pub use self::capacity::HttpCapacityOptions;
pub use self::options::{ReloadableRuntimeConfig, RuntimeOptions};
pub use self::spawn::spawn;
pub(super) use self::spawn::observe_reload_stage_elapsed;
pub(super) use self::plan::{
    ListenerPlan, ListenerProtocol, PlannedListener, TlsIdentity,
    TlsMaterial,
};
#[cfg(test)]
pub(super) use self::plan::{build_listener_plan, build_listener_plan_with_bind_checker};
#[cfg(test)]
pub(super) use self::plan::build_listener_plan_with_bind_checker_for_runtime;
pub(super) use self::updates::{listener_updates, listener_updates_with_force_reload};
#[cfg(test)]
use self::listener_plan::materialize_tls_assets;
#[cfg(test)]
use self::listener_plan::wildcard_hostname_matches;
use self::listener_plan::{
    RuntimeListener, RuntimeListenerProtocol, RuntimePlan, TlsAssetWriteStats,
    cleanup_unused_tls_assets_in_dir,
    listener_bind_addrs, materialize_runtime_plan, materialize_tls_assets_in_dir,
    ordered_tls_identity_candidates, referenced_tls_asset_prefixes, tcp_socket_options_for_bind,
};
use self::listener_set::{ActiveServer};
pub use self::server::{AcceptedHttpApp, build_http_app, process_accepted_stream};
#[cfg(test)]
use self::server::{
    listener_port_hint, plain_http_server_options, start_server, start_server_with_overload_stats,
};
use self::server::{start_server_with_asset_root, stop_server};

#[cfg(test)]
mod tests;
