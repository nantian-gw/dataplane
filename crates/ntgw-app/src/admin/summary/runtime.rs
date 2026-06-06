use std::time::{Duration, SystemTime, UNIX_EPOCH};

use ntgw_ir::Snapshot;

use super::super::filters::{
    is_plain_http_runtime_listener, is_pure_stream_runtime_listener, is_tls_runtime_listener,
};

#[derive(Debug, Default, Clone, Copy)]
pub(crate) struct SessionPersistenceUsage {
    pub(crate) route_rules: usize,
    pub(crate) backend_policies: usize,
}

impl SessionPersistenceUsage {
    fn total(self) -> usize {
        self.route_rules + self.backend_policies
    }

    pub(crate) fn active(self) -> bool {
        self.total() > 0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CurrentSnapshotState {
    pub(crate) status: &'static str,
    pub(crate) accepted: bool,
    pub(crate) rejected: bool,
    pub(crate) serving_last_good_snapshot: bool,
    pub(crate) last_good_snapshot_version: String,
    pub(crate) fallback_state: &'static str,
    pub(crate) rejection_version: String,
    pub(crate) rejection_runtime: String,
    pub(crate) rejection_message: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RuntimePlaneState {
    pub(crate) required: bool,
    pub(crate) status: &'static str,
    pub(crate) accepted: bool,
    pub(crate) rejected: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ReadinessState {
    pub(crate) ready: bool,
    pub(crate) state: &'static str,
    pub(crate) reason: &'static str,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct LivenessState {
    pub(crate) live: bool,
    pub(crate) state: &'static str,
    pub(crate) reason: &'static str,
}

pub(crate) fn build_current_snapshot_state(
    snapshot: &Snapshot,
    runtime: &ntgw_observability::RuntimeStatsSnapshot,
    xds: &ntgw_xds::ClientStatsSnapshot,
) -> CurrentSnapshotState {
    if snapshot.id.is_empty() {
        return CurrentSnapshotState {
            status: "warming",
            accepted: false,
            rejected: false,
            serving_last_good_snapshot: false,
            last_good_snapshot_version: xds.last_snapshot_version.clone(),
            fallback_state: "warming",
            rejection_version: String::new(),
            rejection_runtime: String::new(),
            rejection_message: String::new(),
        };
    }

    let http_rejected = runtime.http_last_reload_failure_version == snapshot.id
        && !runtime.http_last_reload_failure_message.is_empty();
    let tls_rejected = runtime.tls_last_reload_failure_version == snapshot.id
        && !runtime.tls_last_reload_failure_message.is_empty();
    let stream_rejected = runtime.stream_last_reload_failure_version == snapshot.id
        && !runtime.stream_last_reload_failure_message.is_empty();
    let rejected = http_rejected || tls_rejected || stream_rejected;
    let rejection_runtime = match (http_rejected, tls_rejected, stream_rejected) {
        (true, true, true) => "http+tls+stream",
        (true, true, false) => "http+tls",
        (true, false, true) => "http+stream",
        (false, true, true) => "tls+stream",
        (true, false, false) => "http",
        (false, true, false) => "tls",
        (false, false, true) => "stream",
        (false, false, false) => "",
    };

    let rejection_message =
        format_runtime_rejection_message(runtime, http_rejected, tls_rejected, stream_rejected);

    CurrentSnapshotState {
        status: if rejected { "rejected" } else { "accepted" },
        accepted: !rejected,
        rejected,
        serving_last_good_snapshot: rejected && !xds.last_snapshot_version.is_empty(),
        last_good_snapshot_version: xds.last_snapshot_version.clone(),
        fallback_state: match (rejected, xds.last_snapshot_version.is_empty()) {
            (true, false) => "last-good-rejected",
            _ => "none",
        },
        rejection_version: if rejected {
            snapshot.id.clone()
        } else {
            String::new()
        },
        rejection_runtime: rejection_runtime.to_string(),
        rejection_message,
    }
}

pub(crate) fn build_readiness_state(
    snapshot: &Snapshot,
    runtime: &ntgw_observability::RuntimeStatsSnapshot,
    xds: &ntgw_xds::ClientStatsSnapshot,
    snapshot_freshness_timeout: Duration,
) -> ReadinessState {
    if runtime.supervisor_shutdown_requested {
        return ReadinessState {
            ready: false,
            state: "not-ready",
            reason: "supervisor-shutting-down",
        };
    }

    let current_snapshot = build_current_snapshot_state(snapshot, runtime, xds);
    let http_runtime = build_runtime_plane_state(
        snapshot_requires_http_runtime(snapshot),
        snapshot.id.as_str(),
        runtime.http_last_reload_attempt_version.as_str(),
        runtime.http_last_good_reload_version.as_str(),
        runtime.http_last_reload_failure_version.as_str(),
        runtime.http_last_reload_failure_message.as_str(),
    );
    let tls_runtime = build_runtime_plane_state(
        snapshot_requires_tls_runtime(snapshot),
        snapshot.id.as_str(),
        runtime.tls_last_reload_attempt_version.as_str(),
        runtime.tls_last_good_reload_version.as_str(),
        runtime.tls_last_reload_failure_version.as_str(),
        runtime.tls_last_reload_failure_message.as_str(),
    );
    let stream_runtime = build_runtime_plane_state(
        snapshot_requires_stream_runtime(snapshot),
        snapshot.id.as_str(),
        runtime.stream_last_reload_attempt_version.as_str(),
        runtime.stream_last_good_reload_version.as_str(),
        runtime.stream_last_reload_failure_version.as_str(),
        runtime.stream_last_reload_failure_message.as_str(),
    );

    if http_runtime.required
        && !runtime.http_runtime_running
        && runtime.http_last_exit_unix_seconds > 0
    {
        return ReadinessState {
            ready: false,
            state: "not-ready",
            reason: "http-runtime-exited",
        };
    }

    if tls_runtime.required
        && !runtime.tls_runtime_running
        && runtime.tls_last_exit_unix_seconds > 0
    {
        return ReadinessState {
            ready: false,
            state: "not-ready",
            reason: "tls-runtime-exited",
        };
    }

    if stream_runtime.required
        && !runtime.stream_runtime_running
        && runtime.stream_last_exit_unix_seconds > 0
    {
        return ReadinessState {
            ready: false,
            state: "not-ready",
            reason: "stream-runtime-exited",
        };
    }

    if !snapshot.id.is_empty()
        && !xds.stream_connected
        && xds.last_control_plane_contact_unix_seconds > 0
        && epoch_seconds().saturating_sub(xds.last_control_plane_contact_unix_seconds)
            >= snapshot_freshness_timeout.as_secs()
    {
        return ReadinessState {
            ready: false,
            state: "not-ready",
            reason: "xds-snapshot-stale",
        };
    }

    let serving_current = (!http_runtime.required || http_runtime.accepted)
        && (!tls_runtime.required || tls_runtime.accepted)
        && (!stream_runtime.required || stream_runtime.accepted)
        && !snapshot.id.is_empty()
        && current_snapshot.accepted;
    if serving_current {
        return ReadinessState {
            ready: true,
            state: "serving-current",
            reason: "current-snapshot-serving",
        };
    }

    let has_last_good = !runtime.http_last_good_reload_version.is_empty()
        || !runtime.tls_last_good_reload_version.is_empty()
        || !runtime.stream_last_good_reload_version.is_empty()
        || !current_snapshot.last_good_snapshot_version.is_empty();
    if has_last_good {
        return ReadinessState {
            ready: true,
            state: "serving-last-good",
            reason: if current_snapshot.rejected {
                "serving-last-good-after-rejection"
            } else {
                "serving-last-good-while-current-pending"
            },
        };
    }

    if snapshot.id.is_empty() {
        return ReadinessState {
            ready: false,
            state: "warming",
            reason: "waiting-for-first-snapshot",
        };
    }

    ReadinessState {
        ready: false,
        state: "not-ready",
        reason: if current_snapshot.rejected {
            "current-snapshot-rejected-without-last-good"
        } else {
            "current-snapshot-pending-without-last-good"
        },
    }
}

pub(crate) fn build_liveness_state(
    snapshot: &Snapshot,
    runtime: &ntgw_observability::RuntimeStatsSnapshot,
) -> LivenessState {
    if runtime.supervisor_shutdown_requested {
        return LivenessState {
            live: false,
            state: "not-live",
            reason: "supervisor-shutting-down",
        };
    }

    if snapshot_requires_http_runtime(snapshot)
        && !runtime.http_runtime_running
        && runtime.http_last_exit_unix_seconds > 0
    {
        return LivenessState {
            live: false,
            state: "not-live",
            reason: "http-runtime-exited",
        };
    }

    if snapshot_requires_tls_runtime(snapshot)
        && !runtime.tls_runtime_running
        && runtime.tls_last_exit_unix_seconds > 0
    {
        return LivenessState {
            live: false,
            state: "not-live",
            reason: "tls-runtime-exited",
        };
    }

    if snapshot_requires_stream_runtime(snapshot)
        && !runtime.stream_runtime_running
        && runtime.stream_last_exit_unix_seconds > 0
    {
        return LivenessState {
            live: false,
            state: "not-live",
            reason: "stream-runtime-exited",
        };
    }

    if runtime.supervisor_last_exit_unix_seconds > 0 && !runtime.supervisor_running {
        return LivenessState {
            live: false,
            state: "not-live",
            reason: "supervisor-exited",
        };
    }

    LivenessState {
        live: true,
        state: "alive",
        reason: "process-running",
    }
}

pub(super) fn format_runtime_failures(
    failures: &[ntgw_observability::RuntimeListenerFailure],
) -> String {
    if failures.is_empty() {
        return String::new();
    }

    failures
        .iter()
        .map(|failure| format!("{}: {}", failure.listener, failure.message))
        .collect::<Vec<_>>()
        .join("; ")
}

pub(crate) fn build_runtime_plane_state(
    required: bool,
    snapshot_version: &str,
    current_attempt_version: &str,
    last_good_version: &str,
    last_failure_version: &str,
    last_failure_message: &str,
) -> RuntimePlaneState {
    if !required {
        return RuntimePlaneState {
            required,
            status: "idle",
            accepted: false,
            rejected: false,
        };
    }

    if snapshot_version.is_empty() {
        return RuntimePlaneState {
            required,
            status: "warming",
            accepted: false,
            rejected: false,
        };
    }

    if last_failure_version == snapshot_version && !last_failure_message.is_empty() {
        return RuntimePlaneState {
            required,
            status: "rejected",
            accepted: false,
            rejected: true,
        };
    }

    if last_good_version == snapshot_version {
        return RuntimePlaneState {
            required,
            status: "accepted",
            accepted: true,
            rejected: false,
        };
    }

    let _ = current_attempt_version;
    RuntimePlaneState {
        required,
        status: "pending",
        accepted: false,
        rejected: false,
    }
}

pub(crate) fn snapshot_requires_http_runtime(snapshot: &Snapshot) -> bool {
    snapshot
        .listeners
        .iter()
        .any(|listener| is_plain_http_runtime_listener(listener.protocol.as_str()))
}

pub(crate) fn snapshot_requires_tls_runtime(snapshot: &Snapshot) -> bool {
    snapshot
        .listeners
        .iter()
        .any(|listener| is_tls_runtime_listener(listener.protocol.as_str()))
}

pub(crate) fn snapshot_requires_stream_runtime(snapshot: &Snapshot) -> bool {
    snapshot
        .listeners
        .iter()
        .any(|listener| is_pure_stream_runtime_listener(listener.protocol.as_str()))
}

fn format_runtime_rejection_message(
    runtime: &ntgw_observability::RuntimeStatsSnapshot,
    http_rejected: bool,
    tls_rejected: bool,
    stream_rejected: bool,
) -> String {
    let mut segments = Vec::new();
    if http_rejected {
        segments.push(format!(
            "HTTP runtime: {}",
            format_runtime_failures(&runtime.http_current_failures)
        ));
    }
    if tls_rejected {
        segments.push(format!(
            "TLS runtime: {}",
            format_runtime_failures(&runtime.tls_current_failures)
        ));
    }
    if stream_rejected {
        segments.push(format!(
            "stream runtime: {}",
            format_runtime_failures(&runtime.stream_current_failures)
        ));
    }
    segments.join("; ")
}

fn epoch_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|value| value.as_secs())
        .unwrap_or_default()
}
