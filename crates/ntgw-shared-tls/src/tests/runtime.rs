use std::sync::{Arc, Mutex};

use anyhow::Result;
use ntgw_http::{
    AccessLogOptions, RuntimeOptions as HttpRuntimeOptions, SessionPersistenceOptions,
    build_http_app,
};
use ntgw_ir::{Listener, Snapshot, SnapshotSignal, TlsConfig};
use ntgw_observability::{
    ApplyStageRecorder, HttpCircuitBreakerController, HttpRateLimitController,
    RetryBudgetController, RuntimeStats, SharedApplyStageRecorder, SharedTrafficStats,
};
use tokio::{
    io::AsyncWriteExt,
    net::{TcpListener, TcpStream},
    sync::watch,
    time::{Duration, sleep, timeout},
};

use crate::{
    ReloadableRuntimeConfig, RuntimeOptions, listener_plan::build_listener_plan, run,
    runtime::ConnectionConfig,
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
