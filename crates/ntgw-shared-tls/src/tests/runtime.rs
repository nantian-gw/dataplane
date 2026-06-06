use std::sync::{Arc, Mutex};

use ntgw_http::{
    build_http_app, AccessLogOptions, RuntimeOptions as HttpRuntimeOptions,
    SessionPersistenceOptions,
};
use ntgw_ir::{Listener, Snapshot, SnapshotSignal, TlsConfig};
use ntgw_observability::{
    ApplyStageRecorder, HttpCircuitBreakerController, HttpRateLimitController,
    RetryBudgetController, RuntimeStats, SharedApplyStageRecorder, SharedTrafficStats,
};
use anyhow::Result;
use tokio::{
    io::AsyncWriteExt,
    net::{TcpListener, TcpStream},
    sync::watch,
    time::{sleep, timeout, Duration},
};

use crate::{
    listener_plan::build_listener_plan, run, runtime::ConnectionConfig, ReloadableRuntimeConfig,
    RuntimeOptions,
};

use super::{
    build_client_hello, free_tcp_port, https_request, read_http_headers, shared_tls_snapshot,
};

#[path = "runtime/missing_bind.rs"]
mod missing_bind;
#[path = "runtime/passthrough_reload.rs"]
mod passthrough_reload;
#[path = "runtime/routes.rs"]
mod routes;
