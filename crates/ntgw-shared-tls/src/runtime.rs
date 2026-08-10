#[cfg(unix)]
use std::os::unix::io::AsRawFd;
#[cfg(windows)]
use std::os::windows::io::AsRawSocket;
use std::{
    collections::{BTreeMap, BTreeSet},
    net::SocketAddr,
    sync::Arc,
    time::Instant,
};

use ntgw_http::proxy::GatewayProxyOptions;
use ntgw_http::{AcceptedHttpApp, UpstreamTuningOptions, build_http_app, process_accepted_stream};
use ntgw_ir::{Listener, SharedSnapshot, SharedSnapshotSignal, Snapshot, TlsRouteMode};
use ntgw_observability::{
    HttpAdmissionController, HttpCircuitBreakerController, HttpRateLimitController,
    RetryBudgetController, RuntimeListenerFailure, SharedApplyStageRecorder, SharedOverloadStats,
    SharedRuntimeStats, SharedTrafficStats,
};
use parking_lot::RwLock;
use pingora::protocols::l4::stream::Stream as L4Stream;
use pingora::protocols::{GetSocketDigest, SocketDigest};
use socket2::{Domain, Protocol, Socket, Type};
use tokio::{
    net::{TcpListener, TcpStream},
    sync::watch,
    task::JoinHandle,
    time::{sleep, timeout},
};
use tracing::{error, info, warn};

use crate::{
    RuntimeOptions, SharedTlsError,
    dispatch::{proxy_passthrough, proxy_terminated_stream},
    listener_plan::{ListenerPlan, PlannedSharedTlsBind, build_listener_plan},
    preface::peek_client_hello,
};

mod handshake;
use self::handshake::terminate_tls;

#[derive(Clone)]
pub struct ReloadableRuntimeConfig {
    pub runtime: RuntimeOptions,
    pub http: ntgw_http::ReloadableRuntimeConfig,
    pub stream: ntgw_stream::ReloadableRuntimeConfig,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct ConnectionConfig;

include!("runtime/run.incl.rs");

include!("runtime/binds.incl.rs");

include!("runtime/selection.incl.rs");

#[cfg(test)]
mod tests;
