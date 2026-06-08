use ntgw_ir::{Listener, Snapshot};

use super::{
    super::{
        filters::{
            is_plain_http_runtime_listener, is_pure_stream_runtime_listener,
            is_tls_runtime_listener,
        },
        types::ListenerRuntimeStatus,
    },
    runtime::{
        RuntimePlaneState, build_runtime_plane_state, snapshot_requires_http_runtime,
        snapshot_requires_stream_runtime, snapshot_requires_tls_runtime,
    },
};

pub(super) fn classify_listener_serving_state(
    listener_current_status: &str,
    listener_serving_current_snapshot: bool,
    listener_serving_last_good_snapshot: bool,
) -> &'static str {
    if listener_serving_current_snapshot {
        return match listener_current_status {
            "retained" => "current-retained",
            "accepted" => "current-accepted",
            _ => "none",
        };
    }

    if listener_serving_last_good_snapshot {
        return match listener_current_status {
            "rejected" => "last-good-rejected",
            "stale" => "last-good-stale",
            _ => "none",
        };
    }

    "none"
}

#[allow(clippy::too_many_arguments)]
pub(super) fn classify_listener_recovery_state(
    runtime_required: bool,
    snapshot_version: &str,
    listener_current_status: &str,
    listener_recovered_from_failure: bool,
    listener_awaiting_current_attempt: bool,
    listener_current_attempt_blocked: bool,
    listener_unrecovered_current_snapshot_failure: bool,
    listener_unrecovered_historical_failure: bool,
) -> &'static str {
    if !runtime_required {
        return "idle";
    }

    if snapshot_version.is_empty() {
        return "warming";
    }

    if listener_recovered_from_failure {
        return "recovered";
    }

    if listener_unrecovered_current_snapshot_failure {
        return "unrecovered-current";
    }

    if listener_unrecovered_historical_failure {
        return "unrecovered-historical";
    }

    if listener_awaiting_current_attempt {
        return "awaiting-current";
    }

    if listener_current_attempt_blocked {
        return "blocked-current";
    }

    if listener_current_status == "stale" {
        return "drifted-last-good";
    }

    "steady"
}

pub(crate) fn build_listener_runtime_status(
    listener: &Listener,
    snapshot: &Snapshot,
    runtime: &ntgw_observability::RuntimeStatsSnapshot,
) -> ListenerRuntimeStatus {
    let empty_failures: &[ntgw_observability::RuntimeListenerFailure] = &[];
    let empty_progress = ntgw_observability::RuntimeListenerProgress::default();
    let (runtime_plane, runtime_state, failure_version, current_failures, listener_progress) =
        if is_plain_http_runtime_listener(listener.protocol.as_str()) {
            (
                "http",
                build_runtime_plane_state(
                    snapshot_requires_http_runtime(snapshot),
                    snapshot.id.as_str(),
                    runtime.http_last_reload_attempt_version.as_str(),
                    runtime.http_last_good_reload_version.as_str(),
                    runtime.http_last_reload_failure_version.as_str(),
                    runtime.http_last_reload_failure_message.as_str(),
                ),
                runtime.http_last_reload_failure_version.as_str(),
                runtime.http_current_failures.as_slice(),
                runtime.http_listener_progress.get(&listener.name),
            )
        } else if is_tls_runtime_listener(listener.protocol.as_str()) {
            (
                "tls",
                build_runtime_plane_state(
                    snapshot_requires_tls_runtime(snapshot),
                    snapshot.id.as_str(),
                    runtime.tls_last_reload_attempt_version.as_str(),
                    runtime.tls_last_good_reload_version.as_str(),
                    runtime.tls_last_reload_failure_version.as_str(),
                    runtime.tls_last_reload_failure_message.as_str(),
                ),
                runtime.tls_last_reload_failure_version.as_str(),
                runtime.tls_current_failures.as_slice(),
                runtime.tls_listener_progress.get(&listener.name),
            )
        } else if is_pure_stream_runtime_listener(listener.protocol.as_str()) {
            (
                "stream",
                build_runtime_plane_state(
                    snapshot_requires_stream_runtime(snapshot),
                    snapshot.id.as_str(),
                    runtime.stream_last_reload_attempt_version.as_str(),
                    runtime.stream_last_good_reload_version.as_str(),
                    runtime.stream_last_reload_failure_version.as_str(),
                    runtime.stream_last_reload_failure_message.as_str(),
                ),
                runtime.stream_last_reload_failure_version.as_str(),
                runtime.stream_current_failures.as_slice(),
                runtime.stream_listener_progress.get(&listener.name),
            )
        } else {
            (
                "none",
                RuntimePlaneState {
                    required: false,
                    status: "idle",
                    accepted: false,
                    rejected: false,
                },
                "",
                empty_failures,
                None,
            )
        };
    let listener_progress = listener_progress.unwrap_or(&empty_progress);
    let listener_current_failure = runtime_state.rejected
        && current_failures
            .iter()
            .any(|failure| failure.listener == listener.name);
    let current_failure = current_failures
        .iter()
        .find(|failure| failure.listener == listener.name);
    let listener_serving_current_snapshot =
        !snapshot.id.is_empty() && listener_progress.last_good_version == snapshot.id;
    let listener_serving_last_good_snapshot = !listener_progress.last_good_version.is_empty()
        && listener_progress.last_good_version != snapshot.id;
    let current_event = listener_progress
        .recent_events
        .iter()
        .find(|event| event.version == snapshot.id);
    let listener_current_status = if !runtime_state.required {
        "idle"
    } else if snapshot.id.is_empty() {
        "warming"
    } else if listener_current_failure {
        "rejected"
    } else if listener_serving_current_snapshot {
        match current_event.map(|event| event.status.as_str()) {
            Some("retained") => "retained",
            _ => "accepted",
        }
    } else if listener_serving_last_good_snapshot {
        "stale"
    } else {
        "pending"
    };
    let listener_current_accepted = listener_current_status == "accepted";
    let listener_current_retained = listener_current_status == "retained";
    let listener_current_rejected = listener_current_status == "rejected";
    let listener_current_stale = listener_current_status == "stale";
    let listener_serving_state = classify_listener_serving_state(
        listener_current_status,
        listener_serving_current_snapshot,
        listener_serving_last_good_snapshot,
    );
    let listener_has_ever_failed = listener_progress.failures > 0
        || !listener_progress.last_failure_version.is_empty()
        || !listener_progress.last_failure_message.is_empty();
    let listener_recovered_from_failure = listener_has_ever_failed
        && !listener_current_failure
        && !listener_progress.last_good_version.is_empty()
        && (listener_progress.last_good_version != listener_progress.last_failure_version
            || listener_progress.last_apply_unix_seconds
                > listener_progress.last_failure_unix_seconds);
    let listener_recovery_version = if listener_recovered_from_failure {
        listener_progress.last_good_version.clone()
    } else {
        String::new()
    };
    let listener_recovery_unix_seconds = if listener_recovered_from_failure {
        listener_progress.last_apply_unix_seconds
    } else {
        0
    };
    let mut listener_attention_reasons = Vec::new();
    if listener_current_status == "pending" {
        listener_attention_reasons.push("pending".to_string());
    }
    if listener_current_status == "rejected" {
        listener_attention_reasons.push("rejected".to_string());
    }
    if listener_current_status == "stale" {
        listener_attention_reasons.push("stale".to_string());
    }
    if listener_has_ever_failed && !listener_recovered_from_failure {
        listener_attention_reasons.push("unrecovered_failure".to_string());
    }
    let listener_attention_required = !listener_attention_reasons.is_empty();
    let listener_awaiting_current_attempt = listener_current_status == "pending"
        && listener_progress.last_attempt_version != snapshot.id;
    let listener_current_attempt_blocked =
        matches!(listener_current_status, "pending" | "rejected")
            && listener_progress.last_attempt_version == snapshot.id;
    let listener_unrecovered_current_snapshot_failure = listener_has_ever_failed
        && !listener_recovered_from_failure
        && listener_progress.last_failure_version == snapshot.id;
    let listener_unrecovered_historical_failure = listener_has_ever_failed
        && !listener_recovered_from_failure
        && listener_progress.last_failure_version != snapshot.id;
    let listener_recovery_state = classify_listener_recovery_state(
        runtime_state.required,
        snapshot.id.as_str(),
        listener_current_status,
        listener_recovered_from_failure,
        listener_awaiting_current_attempt,
        listener_current_attempt_blocked,
        listener_unrecovered_current_snapshot_failure,
        listener_unrecovered_historical_failure,
    );

    ListenerRuntimeStatus {
        listener: listener.clone(),
        runtime_id: snapshot
            .listener_runtime_id(listener.name.as_str())
            .map(|id| id.to_string()),
        runtime_plane: runtime_plane.to_string(),
        runtime_required: runtime_state.required,
        runtime_current_status: runtime_state.status.to_string(),
        runtime_current_accepted: runtime_state.accepted,
        runtime_current_rejected: runtime_state.rejected,
        listener_current_status: listener_current_status.to_string(),
        listener_current_accepted,
        listener_current_retained,
        listener_current_rejected,
        listener_current_stale,
        listener_serving_state: listener_serving_state.to_string(),
        listener_recovery_state: listener_recovery_state.to_string(),
        listener_attention_required,
        listener_attention_reasons,
        listener_current_failure,
        listener_awaiting_current_attempt,
        listener_current_attempt_blocked,
        listener_unrecovered_current_snapshot_failure,
        listener_unrecovered_historical_failure,
        listener_current_failure_version: current_failure
            .map(|_| failure_version.to_string())
            .unwrap_or_default(),
        listener_current_failure_message: current_failure
            .map(|failure| failure.message.clone())
            .unwrap_or_default(),
        listener_attempts: listener_progress.attempts,
        listener_failures: listener_progress.failures,
        listener_last_attempt_version: listener_progress.last_attempt_version.clone(),
        listener_last_good_version: listener_progress.last_good_version.clone(),
        listener_last_failure_version: listener_progress.last_failure_version.clone(),
        listener_last_failure_message: listener_progress.last_failure_message.clone(),
        listener_last_apply_unix_seconds: listener_progress.last_apply_unix_seconds,
        listener_last_failure_unix_seconds: listener_progress.last_failure_unix_seconds,
        listener_has_ever_failed,
        listener_recovered_from_failure,
        listener_recovery_version,
        listener_recovery_unix_seconds,
        listener_serving_version: listener_progress.last_good_version.clone(),
        listener_serving_current_snapshot,
        listener_serving_last_good_snapshot,
        listener_recent_events: listener_progress.recent_events.clone(),
    }
}
