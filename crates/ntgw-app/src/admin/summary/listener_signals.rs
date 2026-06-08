use serde_json::{Map, Value, json};

mod fields;
mod helpers;

use self::{
    fields::{
        insert_name_list, insert_named_fields, insert_named_value, insert_overview_fields,
        insert_plane_named_fields, insert_plane_suffix_fields, insert_runtime_id_list,
    },
    helpers::{
        collect_named, collect_plane_named, count_field, count_matching, filter_non_empty_names,
        has_attention_reason,
    },
};

use super::{
    super::types::ListenerRuntimeStatus,
    listener_overview::{
        classify_listener_attention_overview, classify_listener_convergence_overview,
        classify_listener_failure_recovery_overview, recommended_listener_attention_count,
        recommended_listener_attention_filter, recommended_listener_attention_reason,
        recommended_listener_convergence_count, recommended_listener_convergence_filter,
        recommended_listener_convergence_reason, recommended_listener_failure_recovery_count,
        recommended_listener_failure_recovery_filter, recommended_listener_failure_recovery_reason,
        recommended_listener_status_path, severity_from_level, severity_to_level,
    },
};

pub(super) struct ListenerSignalSummary {
    pub(super) top_level_fields: Map<String, Value>,
    pub(super) listener_state_overview: Value,
    pub(super) listener_overviews: Value,
    pub(super) pending_warning_names: Vec<String>,
    pub(super) rejected_warning_names: Vec<String>,
    pub(super) stale_warning_names: Vec<String>,
    pub(super) unrecovered_failure_warning_names: Vec<String>,
}

pub(super) fn build_listener_signal_summary(
    listener_runtime_statuses: &[ListenerRuntimeStatus],
    snapshot_version: &str,
) -> ListenerSignalSummary {
    let current_idle_count = count_matching(listener_runtime_statuses, |status| {
        status.listener_current_status == "idle"
    });
    let current_warming_count = count_matching(listener_runtime_statuses, |status| {
        status.listener_current_status == "warming"
    });
    let pending = collect_named(listener_runtime_statuses, true, |status| {
        status.listener_current_status == "pending"
    });
    let accepted = collect_named(listener_runtime_statuses, true, |status| {
        status.listener_current_status == "accepted"
    });
    let retained = collect_named(listener_runtime_statuses, true, |status| {
        status.listener_current_status == "retained"
    });
    let rejected = collect_named(listener_runtime_statuses, true, |status| {
        status.listener_current_status == "rejected"
    });
    let stale = collect_named(listener_runtime_statuses, true, |status| {
        status.listener_current_status == "stale"
    });

    let convergence_blocked = collect_plane_named(listener_runtime_statuses, false, |status| {
        matches!(
            status.listener_current_status.as_str(),
            "pending" | "rejected" | "stale"
        )
    });
    let apply_blocked = collect_plane_named(listener_runtime_statuses, false, |status| {
        matches!(
            status.listener_current_status.as_str(),
            "pending" | "rejected"
        )
    });
    let awaiting_current_attempt =
        collect_plane_named(listener_runtime_statuses, false, |status| {
            status.listener_current_status == "pending"
                && status.listener_last_attempt_version != snapshot_version
        });
    let current_attempt_blocked = collect_plane_named(listener_runtime_statuses, false, |status| {
        matches!(
            status.listener_current_status.as_str(),
            "pending" | "rejected"
        ) && status.listener_last_attempt_version == snapshot_version
    });
    let serving_drift = collect_plane_named(listener_runtime_statuses, false, |status| {
        status.listener_current_status == "stale"
    });
    let serving_current_snapshot = collect_named(listener_runtime_statuses, true, |status| {
        status.listener_serving_current_snapshot
    });
    let serving_last_good_snapshot = collect_named(listener_runtime_statuses, true, |status| {
        status.listener_serving_last_good_snapshot
    });
    let has_ever_failed = collect_named(listener_runtime_statuses, true, |status| {
        status.listener_has_ever_failed
    });
    let recovered_from_failure = collect_plane_named(listener_runtime_statuses, true, |status| {
        status.listener_recovered_from_failure
    });
    let unrecovered_failure = collect_plane_named(listener_runtime_statuses, true, |status| {
        status.listener_has_ever_failed && !status.listener_recovered_from_failure
    });
    let unrecovered_current_snapshot_failure =
        collect_plane_named(listener_runtime_statuses, false, |status| {
            status.listener_has_ever_failed
                && !status.listener_recovered_from_failure
                && status.listener_last_failure_version == snapshot_version
        });
    let unrecovered_historical_failure =
        collect_plane_named(listener_runtime_statuses, false, |status| {
            status.listener_has_ever_failed
                && !status.listener_recovered_from_failure
                && status.listener_last_failure_version != snapshot_version
        });
    let risk_pending_unrecovered = collect_named(listener_runtime_statuses, false, |status| {
        status.listener_current_status == "pending"
            && status.listener_has_ever_failed
            && !status.listener_recovered_from_failure
    });
    let risk_rejected_unrecovered = collect_named(listener_runtime_statuses, false, |status| {
        status.listener_current_status == "rejected"
            && status.listener_has_ever_failed
            && !status.listener_recovered_from_failure
    });
    let risk_stale_unrecovered = collect_named(listener_runtime_statuses, false, |status| {
        status.listener_current_status == "stale"
            && status.listener_has_ever_failed
            && !status.listener_recovered_from_failure
    });
    let attention_required = collect_plane_named(listener_runtime_statuses, false, |status| {
        status.listener_attention_required
    });
    let attention_pending = collect_named(listener_runtime_statuses, false, |status| {
        has_attention_reason(status, "pending")
    });
    let attention_rejected = collect_named(listener_runtime_statuses, false, |status| {
        has_attention_reason(status, "rejected")
    });
    let attention_stale = collect_named(listener_runtime_statuses, false, |status| {
        has_attention_reason(status, "stale")
    });
    let attention_unrecovered_failure = collect_named(listener_runtime_statuses, false, |status| {
        has_attention_reason(status, "unrecovered_failure")
    });

    let (convergence_severity, convergence_primary_signal) = classify_listener_convergence_overview(
        awaiting_current_attempt.total.count,
        current_attempt_blocked.total.count,
        serving_drift.total.count,
    );
    let convergence_severity_level = severity_to_level(convergence_severity);
    let convergence_recommended_filter =
        recommended_listener_convergence_filter(convergence_primary_signal);
    let convergence_recommended_path =
        recommended_listener_status_path(convergence_recommended_filter);
    let convergence_recommended_reason =
        recommended_listener_convergence_reason(convergence_primary_signal);
    let convergence_recommended_count = recommended_listener_convergence_count(
        convergence_primary_signal,
        awaiting_current_attempt.total.count,
        current_attempt_blocked.total.count,
        serving_drift.total.count,
        attention_required.total.count,
    );

    let (failure_recovery_severity, failure_recovery_primary_signal) =
        classify_listener_failure_recovery_overview(
            unrecovered_current_snapshot_failure.total.count,
            unrecovered_historical_failure.total.count,
        );
    let failure_recovery_severity_level = severity_to_level(failure_recovery_severity);
    let failure_recovery_recommended_filter =
        recommended_listener_failure_recovery_filter(failure_recovery_primary_signal);
    let failure_recovery_recommended_path =
        recommended_listener_status_path(failure_recovery_recommended_filter);
    let failure_recovery_recommended_reason =
        recommended_listener_failure_recovery_reason(failure_recovery_primary_signal);
    let failure_recovery_recommended_count = recommended_listener_failure_recovery_count(
        failure_recovery_primary_signal,
        unrecovered_current_snapshot_failure.total.count,
        unrecovered_historical_failure.total.count,
        unrecovered_failure.total.count,
    );

    let (attention_severity, attention_primary_signal) = classify_listener_attention_overview(
        attention_pending.count,
        attention_rejected.count,
        attention_stale.count,
        attention_unrecovered_failure.count,
    );
    let attention_severity_level = severity_to_level(attention_severity);
    let attention_recommended_filter =
        recommended_listener_attention_filter(attention_primary_signal);
    let attention_recommended_path = recommended_listener_status_path(attention_recommended_filter);
    let attention_recommended_reason =
        recommended_listener_attention_reason(attention_primary_signal);
    let attention_recommended_count = recommended_listener_attention_count(
        attention_primary_signal,
        attention_pending.count,
        attention_rejected.count,
        attention_stale.count,
        attention_unrecovered_failure.count,
        attention_required.total.count,
    );

    let listener_state_overview = json!({
        "schemaVersion": 1,
        "resourceType": "listener",
        "statusEndpoint": "/v1/listener-statuses",
        "current": {
            "idle": current_idle_count,
            "warming": current_warming_count,
            "pending": pending.count,
            "accepted": accepted.count,
            "retained": retained.count,
            "rejected": rejected.count,
            "stale": stale.count,
        },
        "serving": {
            "currentSnapshot": serving_current_snapshot.count,
            "lastGoodSnapshot": serving_last_good_snapshot.count,
            "drift": serving_drift.total.count,
        }
    });
    let listener_convergence_overview = json!({
        "schemaVersion": 1,
        "resourceType": "listener",
        "statusEndpoint": "/v1/listener-statuses",
        "status": {
            "severity": convergence_severity,
            "severityLevel": convergence_severity_level,
            "primarySignal": convergence_primary_signal,
        },
        "severity": convergence_severity,
        "severityLevel": convergence_severity_level,
        "primarySignal": convergence_primary_signal,
        "drilldown": {
            "filter": convergence_recommended_filter,
            "path": convergence_recommended_path,
            "reason": convergence_recommended_reason,
            "recommendedCount": convergence_recommended_count,
        },
        "recommendedFilter": convergence_recommended_filter,
        "recommendedPath": convergence_recommended_path,
        "recommendedReason": convergence_recommended_reason,
        "recommendedCount": convergence_recommended_count,
        "counts": {
            "recommended": convergence_recommended_count,
            "blocked": convergence_blocked.total.count,
            "awaitingCurrentAttempt": awaiting_current_attempt.total.count,
            "currentAttemptBlocked": current_attempt_blocked.total.count,
            "servingDrift": serving_drift.total.count,
        },
        "blockedCount": convergence_blocked.total.count,
        "awaitingCurrentAttemptCount": awaiting_current_attempt.total.count,
        "currentAttemptBlockedCount": current_attempt_blocked.total.count,
        "servingDriftCount": serving_drift.total.count,
    });
    let listener_failure_recovery_overview = json!({
        "schemaVersion": 1,
        "resourceType": "listener",
        "statusEndpoint": "/v1/listener-statuses",
        "status": {
            "severity": failure_recovery_severity,
            "severityLevel": failure_recovery_severity_level,
            "primarySignal": failure_recovery_primary_signal,
        },
        "severity": failure_recovery_severity,
        "severityLevel": failure_recovery_severity_level,
        "primarySignal": failure_recovery_primary_signal,
        "drilldown": {
            "filter": failure_recovery_recommended_filter,
            "path": failure_recovery_recommended_path,
            "reason": failure_recovery_recommended_reason,
            "recommendedCount": failure_recovery_recommended_count,
        },
        "recommendedFilter": failure_recovery_recommended_filter,
        "recommendedPath": failure_recovery_recommended_path,
        "recommendedReason": failure_recovery_recommended_reason,
        "recommendedCount": failure_recovery_recommended_count,
        "counts": {
            "recommended": failure_recovery_recommended_count,
            "unrecovered": unrecovered_failure.total.count,
            "currentSnapshotFailure": unrecovered_current_snapshot_failure.total.count,
            "historicalFailure": unrecovered_historical_failure.total.count,
        },
        "unrecoveredCount": unrecovered_failure.total.count,
        "currentSnapshotFailureCount": unrecovered_current_snapshot_failure.total.count,
        "historicalFailureCount": unrecovered_historical_failure.total.count,
    });
    let listener_attention_overview = json!({
        "schemaVersion": 1,
        "resourceType": "listener",
        "statusEndpoint": "/v1/listener-statuses",
        "status": {
            "severity": attention_severity,
            "severityLevel": attention_severity_level,
            "primarySignal": attention_primary_signal,
        },
        "severity": attention_severity,
        "severityLevel": attention_severity_level,
        "primarySignal": attention_primary_signal,
        "drilldown": {
            "filter": attention_recommended_filter,
            "path": attention_recommended_path,
            "reason": attention_recommended_reason,
            "recommendedCount": attention_recommended_count,
        },
        "recommendedFilter": attention_recommended_filter,
        "recommendedPath": attention_recommended_path,
        "recommendedReason": attention_recommended_reason,
        "recommendedCount": attention_recommended_count,
        "counts": {
            "recommended": attention_recommended_count,
            "required": attention_required.total.count,
            "pending": attention_pending.count,
            "rejected": attention_rejected.count,
            "stale": attention_stale.count,
            "unrecoveredFailure": attention_unrecovered_failure.count,
        },
        "requiredCount": attention_required.total.count,
        "pendingCount": attention_pending.count,
        "rejectedCount": attention_rejected.count,
        "staleCount": attention_stale.count,
        "unrecoveredFailureCount": attention_unrecovered_failure.count,
    });
    let listener_overview_worst_severity_level = convergence_severity_level
        .max(failure_recovery_severity_level)
        .max(attention_severity_level);
    let listener_overviews = json!({
        "schemaVersion": 1,
        "resourceType": "listener",
        "statusEndpoint": "/v1/listener-statuses",
        "summary": {
            "overviewCount": 3,
            "worstSeverity": severity_from_level(listener_overview_worst_severity_level),
            "worstSeverityLevel": listener_overview_worst_severity_level,
            "statuses": {
                "convergence": {
                    "severity": convergence_severity,
                    "severityLevel": convergence_severity_level,
                    "recommendedCount": convergence_recommended_count,
                },
                "failureRecovery": {
                    "severity": failure_recovery_severity,
                    "severityLevel": failure_recovery_severity_level,
                    "recommendedCount": failure_recovery_recommended_count,
                },
                "attention": {
                    "severity": attention_severity,
                    "severityLevel": attention_severity_level,
                    "recommendedCount": attention_recommended_count,
                }
            }
        },
        "overviewKeys": [
            "summary",
            "convergence",
            "failureRecovery",
            "attention",
        ],
        "convergence": listener_convergence_overview.clone(),
        "failureRecovery": listener_failure_recovery_overview.clone(),
        "attention": listener_attention_overview.clone(),
    });

    let serving_state_counts = json!({
        "none": count_field(listener_runtime_statuses, |status| status.listener_serving_state.as_str(), "none"),
        "currentAccepted": count_field(listener_runtime_statuses, |status| status.listener_serving_state.as_str(), "current-accepted"),
        "currentRetained": count_field(listener_runtime_statuses, |status| status.listener_serving_state.as_str(), "current-retained"),
        "lastGoodRejected": count_field(listener_runtime_statuses, |status| status.listener_serving_state.as_str(), "last-good-rejected"),
        "lastGoodStale": count_field(listener_runtime_statuses, |status| status.listener_serving_state.as_str(), "last-good-stale"),
    });
    let recovery_state_counts = json!({
        "idle": count_field(listener_runtime_statuses, |status| status.listener_recovery_state.as_str(), "idle"),
        "warming": count_field(listener_runtime_statuses, |status| status.listener_recovery_state.as_str(), "warming"),
        "steady": count_field(listener_runtime_statuses, |status| status.listener_recovery_state.as_str(), "steady"),
        "recovered": count_field(listener_runtime_statuses, |status| status.listener_recovery_state.as_str(), "recovered"),
        "awaitingCurrent": count_field(listener_runtime_statuses, |status| status.listener_recovery_state.as_str(), "awaiting-current"),
        "blockedCurrent": count_field(listener_runtime_statuses, |status| status.listener_recovery_state.as_str(), "blocked-current"),
        "unrecoveredCurrent": count_field(listener_runtime_statuses, |status| status.listener_recovery_state.as_str(), "unrecovered-current"),
        "unrecoveredHistorical": count_field(listener_runtime_statuses, |status| status.listener_recovery_state.as_str(), "unrecovered-historical"),
        "driftedLastGood": count_field(listener_runtime_statuses, |status| status.listener_recovery_state.as_str(), "drifted-last-good"),
    });

    let mut top_level_fields = Map::new();
    insert_named_value(
        &mut top_level_fields,
        "listenerCurrentIdleCount",
        current_idle_count,
    );
    insert_named_value(
        &mut top_level_fields,
        "listenerCurrentWarmingCount",
        current_warming_count,
    );
    insert_named_fields(&mut top_level_fields, "listenerCurrentPending", &pending);
    insert_named_fields(&mut top_level_fields, "listenerCurrentAccepted", &accepted);
    insert_named_fields(&mut top_level_fields, "listenerCurrentRetained", &retained);
    insert_named_fields(&mut top_level_fields, "listenerCurrentRejected", &rejected);
    insert_named_fields(&mut top_level_fields, "listenerCurrentStale", &stale);
    insert_plane_named_fields(
        &mut top_level_fields,
        "listenerConvergenceBlocked",
        &convergence_blocked,
    );
    insert_overview_fields(
        &mut top_level_fields,
        "listenerConvergence",
        convergence_severity,
        convergence_severity_level,
        convergence_primary_signal,
        convergence_recommended_filter,
        convergence_recommended_path.as_str(),
        convergence_recommended_reason,
        convergence_recommended_count,
        &listener_convergence_overview,
    );
    insert_plane_named_fields(
        &mut top_level_fields,
        "listenerApplyBlocked",
        &apply_blocked,
    );
    insert_plane_named_fields(
        &mut top_level_fields,
        "listenerAwaitingCurrentAttempt",
        &awaiting_current_attempt,
    );
    insert_plane_named_fields(
        &mut top_level_fields,
        "listenerCurrentAttemptBlocked",
        &current_attempt_blocked,
    );
    insert_plane_named_fields(
        &mut top_level_fields,
        "listenerServingDrift",
        &serving_drift,
    );
    insert_named_value(
        &mut top_level_fields,
        "listenerServingCurrentSnapshotCount",
        serving_current_snapshot.count,
    );
    insert_named_value(
        &mut top_level_fields,
        "listenerServingLastGoodSnapshotCount",
        serving_last_good_snapshot.count,
    );
    top_level_fields.insert(
        "listenerServingStateCounts".to_string(),
        serving_state_counts,
    );
    insert_name_list(
        &mut top_level_fields,
        "listenerServingCurrentSnapshotNames",
        &serving_current_snapshot.names,
    );
    insert_runtime_id_list(
        &mut top_level_fields,
        "listenerServingCurrentSnapshotRuntimeIds",
        &serving_current_snapshot.runtime_ids,
    );
    insert_name_list(
        &mut top_level_fields,
        "listenerServingLastGoodSnapshotNames",
        &serving_last_good_snapshot.names,
    );
    insert_runtime_id_list(
        &mut top_level_fields,
        "listenerServingLastGoodSnapshotRuntimeIds",
        &serving_last_good_snapshot.runtime_ids,
    );
    top_level_fields.insert(
        "listenerStateOverview".to_string(),
        listener_state_overview.clone(),
    );
    insert_named_fields(
        &mut top_level_fields,
        "listenerHasEverFailed",
        &has_ever_failed,
    );
    insert_plane_named_fields(
        &mut top_level_fields,
        "listenerRecoveredFromFailure",
        &recovered_from_failure,
    );
    insert_plane_named_fields(
        &mut top_level_fields,
        "listenerUnrecoveredFailure",
        &unrecovered_failure,
    );
    top_level_fields.insert(
        "listenerRecoveryStateCounts".to_string(),
        recovery_state_counts,
    );
    insert_plane_named_fields(
        &mut top_level_fields,
        "listenerUnrecoveredCurrentSnapshotFailure",
        &unrecovered_current_snapshot_failure,
    );
    insert_plane_named_fields(
        &mut top_level_fields,
        "listenerUnrecoveredHistoricalFailure",
        &unrecovered_historical_failure,
    );
    insert_overview_fields(
        &mut top_level_fields,
        "listenerFailureRecovery",
        failure_recovery_severity,
        failure_recovery_severity_level,
        failure_recovery_primary_signal,
        failure_recovery_recommended_filter,
        failure_recovery_recommended_path.as_str(),
        failure_recovery_recommended_reason,
        failure_recovery_recommended_count,
        &listener_failure_recovery_overview,
    );
    insert_named_fields(
        &mut top_level_fields,
        "listenerRiskPendingUnrecovered",
        &risk_pending_unrecovered,
    );
    insert_named_fields(
        &mut top_level_fields,
        "listenerRiskRejectedUnrecovered",
        &risk_rejected_unrecovered,
    );
    insert_named_fields(
        &mut top_level_fields,
        "listenerRiskStaleUnrecovered",
        &risk_stale_unrecovered,
    );
    insert_named_fields(
        &mut top_level_fields,
        "listenerAttentionRequired",
        &attention_required.total,
    );
    insert_plane_suffix_fields(
        &mut top_level_fields,
        "listenerAttention",
        &attention_required,
    );
    insert_named_fields(
        &mut top_level_fields,
        "listenerAttentionPending",
        &attention_pending,
    );
    insert_named_fields(
        &mut top_level_fields,
        "listenerAttentionRejected",
        &attention_rejected,
    );
    insert_named_fields(
        &mut top_level_fields,
        "listenerAttentionStale",
        &attention_stale,
    );
    insert_named_fields(
        &mut top_level_fields,
        "listenerAttentionUnrecoveredFailure",
        &attention_unrecovered_failure,
    );
    insert_overview_fields(
        &mut top_level_fields,
        "listenerAttention",
        attention_severity,
        attention_severity_level,
        attention_primary_signal,
        attention_recommended_filter,
        attention_recommended_path.as_str(),
        attention_recommended_reason,
        attention_recommended_count,
        &listener_attention_overview,
    );
    top_level_fields.insert("listenerOverviews".to_string(), listener_overviews.clone());

    ListenerSignalSummary {
        top_level_fields,
        listener_state_overview,
        listener_overviews,
        pending_warning_names: filter_non_empty_names(&pending.names),
        rejected_warning_names: filter_non_empty_names(&rejected.names),
        stale_warning_names: filter_non_empty_names(&stale.names),
        unrecovered_failure_warning_names: filter_non_empty_names(&unrecovered_failure.total.names),
    }
}
