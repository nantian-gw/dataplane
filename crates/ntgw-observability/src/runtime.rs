use parking_lot::RwLock;
use std::collections::BTreeMap;
use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicU64, Ordering},
};

use tokio::sync::watch;

pub type SharedRuntimeStats = Arc<RuntimeStats>;
pub type RuntimeApplyEventReceiver = watch::Receiver<Option<RuntimeApplyEvent>>;
const LISTENER_EVENT_HISTORY_LIMIT: usize = 8;

mod helpers;
mod reloads;

use self::helpers::epoch_seconds;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimePlane {
    Http,
    Tls,
    Stream,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeApplyOutcome {
    Applied,
    Rejected,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeApplyEvent {
    pub version: String,
    pub plane: RuntimePlane,
    pub outcome: RuntimeApplyOutcome,
    pub message: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize)]
pub struct RuntimeListenerFailure {
    pub listener: String,
    pub message: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize)]
pub struct RuntimeListenerEvent {
    pub status: String,
    pub version: String,
    pub message: String,
    pub unix_seconds: u64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize)]
pub struct RuntimeListenerProgress {
    pub attempts: u64,
    pub failures: u64,
    pub last_attempt_version: String,
    pub last_good_version: String,
    pub last_failure_version: String,
    pub last_failure_message: String,
    pub last_apply_unix_seconds: u64,
    pub last_failure_unix_seconds: u64,
    pub recent_events: Vec<RuntimeListenerEvent>,
}

#[derive(Debug, Default)]
struct RuntimeStatsInner {
    supervisor_last_shutdown_reason: String,
    supervisor_last_exit_message: String,
    http_last_reload_attempt_version: String,
    http_last_exit_message: String,
    tls_last_reload_attempt_version: String,
    tls_last_exit_message: String,
    http_last_good_reload_version: String,
    tls_last_good_reload_version: String,
    http_last_reload_failure_version: String,
    tls_last_reload_failure_version: String,
    http_last_reload_failure_listener: String,
    tls_last_reload_failure_listener: String,
    http_last_reload_failure_message: String,
    tls_last_reload_failure_message: String,
    http_current_failures: Vec<RuntimeListenerFailure>,
    tls_current_failures: Vec<RuntimeListenerFailure>,
    http_listener_progress: BTreeMap<String, RuntimeListenerProgress>,
    tls_listener_progress: BTreeMap<String, RuntimeListenerProgress>,
    stream_last_reload_attempt_version: String,
    stream_last_exit_message: String,
    stream_last_good_reload_version: String,
    stream_last_reload_failure_version: String,
    stream_last_reload_failure_listener: String,
    stream_last_reload_failure_message: String,
    stream_current_failures: Vec<RuntimeListenerFailure>,
    stream_listener_progress: BTreeMap<String, RuntimeListenerProgress>,
}

#[derive(Debug)]
pub struct RuntimeStats {
    http_listener_reload_failures: AtomicU64,
    tls_listener_reload_failures: AtomicU64,
    stream_listener_reload_failures: AtomicU64,
    http_tls_asset_reuses: AtomicU64,
    supervisor_running: AtomicBool,
    supervisor_shutdown_requested: AtomicBool,
    supervisor_last_exit_unix_seconds: AtomicU64,
    http_runtime_running: AtomicBool,
    tls_runtime_running: AtomicBool,
    http_last_exit_unix_seconds: AtomicU64,
    tls_last_exit_unix_seconds: AtomicU64,
    stream_runtime_running: AtomicBool,
    stream_last_exit_unix_seconds: AtomicU64,
    inner: RwLock<RuntimeStatsInner>,
    apply_event_tx: watch::Sender<Option<RuntimeApplyEvent>>,
}

#[derive(Debug, Clone, Default)]
pub struct RuntimeStatsSnapshot {
    pub http_listener_reload_failures: u64,
    pub tls_listener_reload_failures: u64,
    pub stream_listener_reload_failures: u64,
    pub http_tls_asset_reuses: u64,
    pub supervisor_running: bool,
    pub supervisor_shutdown_requested: bool,
    pub supervisor_last_exit_unix_seconds: u64,
    pub supervisor_last_shutdown_reason: String,
    pub supervisor_last_exit_message: String,
    pub http_runtime_running: bool,
    pub tls_runtime_running: bool,
    pub http_last_exit_unix_seconds: u64,
    pub tls_last_exit_unix_seconds: u64,
    pub http_last_reload_attempt_version: String,
    pub http_last_exit_message: String,
    pub tls_last_reload_attempt_version: String,
    pub tls_last_exit_message: String,
    pub http_last_good_reload_version: String,
    pub tls_last_good_reload_version: String,
    pub http_last_reload_failure_version: String,
    pub tls_last_reload_failure_version: String,
    pub http_last_reload_failure_listener: String,
    pub tls_last_reload_failure_listener: String,
    pub http_last_reload_failure_message: String,
    pub tls_last_reload_failure_message: String,
    pub http_current_failures: Vec<RuntimeListenerFailure>,
    pub tls_current_failures: Vec<RuntimeListenerFailure>,
    pub http_listener_progress: BTreeMap<String, RuntimeListenerProgress>,
    pub tls_listener_progress: BTreeMap<String, RuntimeListenerProgress>,
    pub stream_last_reload_attempt_version: String,
    pub stream_runtime_running: bool,
    pub stream_last_exit_unix_seconds: u64,
    pub stream_last_exit_message: String,
    pub stream_last_good_reload_version: String,
    pub stream_last_reload_failure_version: String,
    pub stream_last_reload_failure_listener: String,
    pub stream_last_reload_failure_message: String,
    pub stream_current_failures: Vec<RuntimeListenerFailure>,
    pub stream_listener_progress: BTreeMap<String, RuntimeListenerProgress>,
}

impl RuntimeStats {
    pub fn shared() -> SharedRuntimeStats {
        Arc::new(Self::default())
    }

    pub fn observe_supervisor_started(&self) {
        self.supervisor_running.store(true, Ordering::Relaxed);
        self.supervisor_shutdown_requested
            .store(false, Ordering::Relaxed);
        let mut inner = self.inner.write();
        inner.supervisor_last_shutdown_reason.clear();
        inner.supervisor_last_exit_message.clear();
    }

    pub fn observe_supervisor_shutdown_requested(&self, reason: &str) {
        self.supervisor_shutdown_requested
            .store(true, Ordering::Relaxed);
        self.inner.write().supervisor_last_shutdown_reason = reason.to_string();
    }

    pub fn observe_supervisor_exited(&self, message: &str) {
        self.supervisor_running.store(false, Ordering::Relaxed);
        self.supervisor_last_exit_unix_seconds
            .store(epoch_seconds(), Ordering::Relaxed);
        self.inner.write().supervisor_last_exit_message = message.to_string();
    }

    pub fn observe_http_listener_reload_success(&self, version: &str) {
        self.observe_http_listener_reload_result(version, &[], &[], &[]);
    }

    pub fn observe_http_runtime_started(&self) {
        self.http_runtime_running.store(true, Ordering::Relaxed);
        self.inner.write().http_last_exit_message.clear();
    }

    pub fn observe_http_runtime_exited(&self, message: &str) {
        self.http_runtime_running.store(false, Ordering::Relaxed);
        self.http_last_exit_unix_seconds
            .store(epoch_seconds(), Ordering::Relaxed);
        self.inner.write().http_last_exit_message = message.to_string();
    }

    pub fn observe_http_tls_asset_reuses(&self, count: u64) {
        if count == 0 {
            return;
        }

        self.http_tls_asset_reuses
            .fetch_add(count, Ordering::Relaxed);
    }

    pub fn observe_tls_listener_reload_success(&self, version: &str) {
        self.observe_tls_listener_reload_result(version, &[], &[], &[]);
    }

    pub fn observe_tls_runtime_started(&self) {
        self.tls_runtime_running.store(true, Ordering::Relaxed);
        self.inner.write().tls_last_exit_message.clear();
    }

    pub fn observe_tls_runtime_exited(&self, message: &str) {
        self.tls_runtime_running.store(false, Ordering::Relaxed);
        self.tls_last_exit_unix_seconds
            .store(epoch_seconds(), Ordering::Relaxed);
        self.inner.write().tls_last_exit_message = message.to_string();
    }

    pub fn observe_stream_listener_reload_success(&self, version: &str) {
        self.observe_stream_listener_reload_result(version, &[], &[], &[]);
    }

    pub fn observe_stream_runtime_started(&self) {
        self.stream_runtime_running.store(true, Ordering::Relaxed);
        self.inner.write().stream_last_exit_message.clear();
    }

    pub fn observe_stream_runtime_exited(&self, message: &str) {
        self.stream_runtime_running.store(false, Ordering::Relaxed);
        self.stream_last_exit_unix_seconds
            .store(epoch_seconds(), Ordering::Relaxed);
        self.inner.write().stream_last_exit_message = message.to_string();
    }

    pub fn subscribe_apply_events(&self) -> RuntimeApplyEventReceiver {
        self.apply_event_tx.subscribe()
    }

    pub fn snapshot(&self) -> RuntimeStatsSnapshot {
        let inner = self.inner.read();
        RuntimeStatsSnapshot {
            http_listener_reload_failures: self
                .http_listener_reload_failures
                .load(Ordering::Relaxed),
            tls_listener_reload_failures: self.tls_listener_reload_failures.load(Ordering::Relaxed),
            stream_listener_reload_failures: self
                .stream_listener_reload_failures
                .load(Ordering::Relaxed),
            http_tls_asset_reuses: self.http_tls_asset_reuses.load(Ordering::Relaxed),
            supervisor_running: self.supervisor_running.load(Ordering::Relaxed),
            supervisor_shutdown_requested: self
                .supervisor_shutdown_requested
                .load(Ordering::Relaxed),
            supervisor_last_exit_unix_seconds: self
                .supervisor_last_exit_unix_seconds
                .load(Ordering::Relaxed),
            supervisor_last_shutdown_reason: inner.supervisor_last_shutdown_reason.clone(),
            supervisor_last_exit_message: inner.supervisor_last_exit_message.clone(),
            http_runtime_running: self.http_runtime_running.load(Ordering::Relaxed),
            tls_runtime_running: self.tls_runtime_running.load(Ordering::Relaxed),
            http_last_exit_unix_seconds: self.http_last_exit_unix_seconds.load(Ordering::Relaxed),
            tls_last_exit_unix_seconds: self.tls_last_exit_unix_seconds.load(Ordering::Relaxed),
            http_last_reload_attempt_version: inner.http_last_reload_attempt_version.clone(),
            http_last_exit_message: inner.http_last_exit_message.clone(),
            tls_last_reload_attempt_version: inner.tls_last_reload_attempt_version.clone(),
            tls_last_exit_message: inner.tls_last_exit_message.clone(),
            http_last_good_reload_version: inner.http_last_good_reload_version.clone(),
            tls_last_good_reload_version: inner.tls_last_good_reload_version.clone(),
            http_last_reload_failure_version: inner.http_last_reload_failure_version.clone(),
            tls_last_reload_failure_version: inner.tls_last_reload_failure_version.clone(),
            http_last_reload_failure_listener: inner.http_last_reload_failure_listener.clone(),
            tls_last_reload_failure_listener: inner.tls_last_reload_failure_listener.clone(),
            http_last_reload_failure_message: inner.http_last_reload_failure_message.clone(),
            tls_last_reload_failure_message: inner.tls_last_reload_failure_message.clone(),
            http_current_failures: inner.http_current_failures.clone(),
            tls_current_failures: inner.tls_current_failures.clone(),
            http_listener_progress: inner.http_listener_progress.clone(),
            tls_listener_progress: inner.tls_listener_progress.clone(),
            stream_last_reload_attempt_version: inner.stream_last_reload_attempt_version.clone(),
            stream_runtime_running: self.stream_runtime_running.load(Ordering::Relaxed),
            stream_last_exit_unix_seconds: self
                .stream_last_exit_unix_seconds
                .load(Ordering::Relaxed),
            stream_last_exit_message: inner.stream_last_exit_message.clone(),
            stream_last_good_reload_version: inner.stream_last_good_reload_version.clone(),
            stream_last_reload_failure_version: inner.stream_last_reload_failure_version.clone(),
            stream_last_reload_failure_listener: inner.stream_last_reload_failure_listener.clone(),
            stream_last_reload_failure_message: inner.stream_last_reload_failure_message.clone(),
            stream_current_failures: inner.stream_current_failures.clone(),
            stream_listener_progress: inner.stream_listener_progress.clone(),
        }
    }
}

impl Default for RuntimeStats {
    fn default() -> Self {
        let (apply_event_tx, _apply_event_rx) = watch::channel(None);

        Self {
            http_listener_reload_failures: AtomicU64::new(0),
            tls_listener_reload_failures: AtomicU64::new(0),
            stream_listener_reload_failures: AtomicU64::new(0),
            http_tls_asset_reuses: AtomicU64::new(0),
            supervisor_running: AtomicBool::new(false),
            supervisor_shutdown_requested: AtomicBool::new(false),
            supervisor_last_exit_unix_seconds: AtomicU64::new(0),
            http_runtime_running: AtomicBool::new(false),
            tls_runtime_running: AtomicBool::new(false),
            http_last_exit_unix_seconds: AtomicU64::new(0),
            tls_last_exit_unix_seconds: AtomicU64::new(0),
            stream_runtime_running: AtomicBool::new(false),
            stream_last_exit_unix_seconds: AtomicU64::new(0),
            inner: RwLock::new(RuntimeStatsInner::default()),
            apply_event_tx,
        }
    }
}

#[cfg(test)]
mod tests;
