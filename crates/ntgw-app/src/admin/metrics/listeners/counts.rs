use super::super::super::{
    summary::{
        build_listener_runtime_status, classify_listener_attention_overview,
        classify_listener_convergence_overview, classify_listener_failure_recovery_overview,
        severity_to_level,
    },
    types::ListenerRuntimeStatus,
};
use super::super::context::MetricsContext;

pub(super) struct ListenerMetricCounts {
    pub(super) listener_runtime_statuses: Vec<ListenerRuntimeStatus>,
    pub(super) listener_serving_none_count: usize,
    pub(super) listener_serving_current_accepted_count: usize,
    pub(super) listener_serving_current_retained_count: usize,
    pub(super) listener_serving_last_good_rejected_count: usize,
    pub(super) listener_serving_last_good_stale_count: usize,
    pub(super) listener_attention_required_count: usize,
    pub(super) listener_attention_pending_count: usize,
    pub(super) listener_attention_rejected_count: usize,
    pub(super) listener_attention_stale_count: usize,
    pub(super) listener_attention_unrecovered_failure_count: usize,
    pub(super) listener_convergence_severity_level: u64,
    pub(super) listener_failure_recovery_severity_level: u64,
    pub(super) listener_attention_severity_level: u64,
}

pub(super) fn collect_listener_metric_counts(ctx: &MetricsContext) -> ListenerMetricCounts {
    let snapshot = &ctx.snapshot;
    let runtime = &ctx.runtime;
    let listener_runtime_statuses = snapshot
        .listeners
        .iter()
        .map(|listener| build_listener_runtime_status(listener, snapshot, runtime))
        .collect::<Vec<_>>();
    let listener_awaiting_current_attempt_count = listener_runtime_statuses
        .iter()
        .filter(|status| {
            status.listener_current_status == "pending"
                && status.listener_last_attempt_version != snapshot.id
        })
        .count();
    let listener_current_attempt_blocked_count = listener_runtime_statuses
        .iter()
        .filter(|status| {
            matches!(
                status.listener_current_status.as_str(),
                "pending" | "rejected"
            ) && status.listener_last_attempt_version == snapshot.id
        })
        .count();
    let listener_serving_drift_count = listener_runtime_statuses
        .iter()
        .filter(|status| status.listener_current_status == "stale")
        .count();
    let listener_serving_none_count = listener_runtime_statuses
        .iter()
        .filter(|status| status.listener_serving_state == "none")
        .count();
    let listener_serving_current_accepted_count = listener_runtime_statuses
        .iter()
        .filter(|status| status.listener_serving_state == "current-accepted")
        .count();
    let listener_serving_current_retained_count = listener_runtime_statuses
        .iter()
        .filter(|status| status.listener_serving_state == "current-retained")
        .count();
    let listener_serving_last_good_rejected_count = listener_runtime_statuses
        .iter()
        .filter(|status| status.listener_serving_state == "last-good-rejected")
        .count();
    let listener_serving_last_good_stale_count = listener_runtime_statuses
        .iter()
        .filter(|status| status.listener_serving_state == "last-good-stale")
        .count();
    let listener_unrecovered_current_snapshot_failure_count = listener_runtime_statuses
        .iter()
        .filter(|status| {
            status.listener_has_ever_failed
                && !status.listener_recovered_from_failure
                && status.listener_last_failure_version == snapshot.id
        })
        .count();
    let listener_unrecovered_historical_failure_count = listener_runtime_statuses
        .iter()
        .filter(|status| {
            status.listener_has_ever_failed
                && !status.listener_recovered_from_failure
                && status.listener_last_failure_version != snapshot.id
        })
        .count();
    let listener_attention_required_count = listener_runtime_statuses
        .iter()
        .filter(|status| status.listener_attention_required)
        .count();
    let listener_attention_pending_count = listener_runtime_statuses
        .iter()
        .filter(|status| {
            status
                .listener_attention_reasons
                .iter()
                .any(|reason| reason == "pending")
        })
        .count();
    let listener_attention_rejected_count = listener_runtime_statuses
        .iter()
        .filter(|status| {
            status
                .listener_attention_reasons
                .iter()
                .any(|reason| reason == "rejected")
        })
        .count();
    let listener_attention_stale_count = listener_runtime_statuses
        .iter()
        .filter(|status| {
            status
                .listener_attention_reasons
                .iter()
                .any(|reason| reason == "stale")
        })
        .count();
    let listener_attention_unrecovered_failure_count = listener_runtime_statuses
        .iter()
        .filter(|status| {
            status
                .listener_attention_reasons
                .iter()
                .any(|reason| reason == "unrecovered_failure")
        })
        .count();
    let (listener_convergence_severity, _) = classify_listener_convergence_overview(
        listener_awaiting_current_attempt_count,
        listener_current_attempt_blocked_count,
        listener_serving_drift_count,
    );
    let (listener_failure_recovery_severity, _) = classify_listener_failure_recovery_overview(
        listener_unrecovered_current_snapshot_failure_count,
        listener_unrecovered_historical_failure_count,
    );
    let (listener_attention_severity, _) = classify_listener_attention_overview(
        listener_attention_pending_count,
        listener_attention_rejected_count,
        listener_attention_stale_count,
        listener_attention_unrecovered_failure_count,
    );

    ListenerMetricCounts {
        listener_runtime_statuses,
        listener_serving_none_count,
        listener_serving_current_accepted_count,
        listener_serving_current_retained_count,
        listener_serving_last_good_rejected_count,
        listener_serving_last_good_stale_count,
        listener_attention_required_count,
        listener_attention_pending_count,
        listener_attention_rejected_count,
        listener_attention_stale_count,
        listener_attention_unrecovered_failure_count,
        listener_convergence_severity_level: severity_to_level(listener_convergence_severity),
        listener_failure_recovery_severity_level: severity_to_level(
            listener_failure_recovery_severity,
        ),
        listener_attention_severity_level: severity_to_level(listener_attention_severity),
    }
}
