use super::*;

#[test]
fn summary_view_distinguishes_current_and_historical_unrecovered_failures() {
    let snapshot = Snapshot {
        id: "v2".to_string(),
        listeners: vec![
            Listener {
                name: "pending-historical".to_string(),
                protocol: "LISTENER_PROTOCOL_HTTP".to_string(),
                ..Listener::default()
            },
            Listener {
                name: "rejected-current".to_string(),
                protocol: "LISTENER_PROTOCOL_HTTP".to_string(),
                ..Listener::default()
            },
            Listener {
                name: "stale-historical".to_string(),
                protocol: "LISTENER_PROTOCOL_HTTP".to_string(),
                ..Listener::default()
            },
        ],
        ..Snapshot::default()
    };
    let shared = Snapshot::shared();
    shared.store(Arc::new(snapshot));
    let runtime = RuntimeStats::shared();
    runtime.observe_http_listener_reload_failure("v1", "pending-historical", "bind conflict");
    runtime.observe_http_listener_reload_result("v1", &["stale-historical".to_string()], &[], &[]);
    runtime.observe_http_listener_reload_failure("v1", "stale-historical", "address in use");
    runtime.observe_http_listener_reload_failure("v2", "rejected-current", "port busy");

    let state = build_state_with_parts(
        test_admin_runtime_config(),
        shared,
        runtime,
        ClientStats::shared(),
    );

    let value = build_summary_value(&state);
    assert_eq!(value["listenerUnrecoveredFailureCount"], 3);
    assert_eq!(value["listenerConvergenceSeverity"], "critical");
    assert_eq!(value["listenerConvergencePrimarySignal"], "mixed");
    assert_eq!(
        value["listenerConvergenceRecommendedFilter"],
        "attentionRequired=true"
    );
    assert_eq!(
        value["listenerConvergenceRecommendedPath"],
        "/v1/listener-statuses?attentionRequired=true"
    );
    assert_eq!(value["listenerUnrecoveredCurrentSnapshotFailureCount"], 1);
    assert_eq!(
        value["listenerUnrecoveredCurrentSnapshotFailureNames"],
        serde_json::json!(["rejected-current"])
    );
    assert_eq!(
        value["listenerUnrecoveredCurrentSnapshotFailureHttpCount"],
        1
    );
    assert_eq!(
        value["listenerUnrecoveredCurrentSnapshotFailureHttpNames"],
        serde_json::json!(["rejected-current"])
    );
    assert_eq!(
        value["listenerUnrecoveredCurrentSnapshotFailureStreamCount"],
        0
    );
    assert_eq!(
        value["listenerUnrecoveredCurrentSnapshotFailureStreamNames"],
        serde_json::json!([])
    );
    assert_eq!(
        value["listenerUnrecoveredCurrentSnapshotFailureNoneCount"],
        0
    );
    assert_eq!(
        value["listenerUnrecoveredCurrentSnapshotFailureNoneNames"],
        serde_json::json!([])
    );
    assert_eq!(value["listenerUnrecoveredHistoricalFailureCount"], 2);
    assert_eq!(
        value["listenerUnrecoveredHistoricalFailureNames"],
        serde_json::json!(["pending-historical", "stale-historical"])
    );
    assert_eq!(value["listenerUnrecoveredHistoricalFailureHttpCount"], 2);
    assert_eq!(
        value["listenerUnrecoveredHistoricalFailureHttpNames"],
        serde_json::json!(["pending-historical", "stale-historical"])
    );
    assert_eq!(value["listenerUnrecoveredHistoricalFailureStreamCount"], 0);
    assert_eq!(
        value["listenerUnrecoveredHistoricalFailureStreamNames"],
        serde_json::json!([])
    );
    assert_eq!(value["listenerUnrecoveredHistoricalFailureNoneCount"], 0);
    assert_eq!(
        value["listenerUnrecoveredHistoricalFailureNoneNames"],
        serde_json::json!([])
    );
    assert_eq!(value["listenerFailureRecoverySeverity"], "critical");
    assert_eq!(value["listenerFailureRecoverySeverityLevel"], 2);
    assert_eq!(value["listenerFailureRecoveryPrimarySignal"], "mixed");
    assert_eq!(value["listenerAttentionSeverity"], "critical");
    assert_eq!(value["listenerAttentionSeverityLevel"], 2);
    assert_eq!(value["listenerAttentionPrimarySignal"], "mixed");
    assert_eq!(
        value["listenerAttentionRecommendedFilter"],
        "attentionRequired=true"
    );
    assert_eq!(
        value["listenerAttentionRecommendedPath"],
        "/v1/listener-statuses?attentionRequired=true"
    );
    assert_eq!(
        value["listenerAttentionRecommendedReason"],
        "inspect listeners currently requiring operator attention across multiple categories"
    );
    assert_eq!(value["listenerAttentionRecommendedCount"], 3);
    assert_eq!(
        value["listenerFailureRecoveryRecommendedFilter"],
        "hasEverFailed=true&recoveredFromFailure=false"
    );
    assert_eq!(
        value["listenerFailureRecoveryRecommendedPath"],
        "/v1/listener-statuses?hasEverFailed=true&recoveredFromFailure=false"
    );
    assert_eq!(
        value["listenerFailureRecoveryRecommendedReason"],
        "inspect listeners that have failed and are not yet recovered"
    );
    assert_eq!(value["listenerFailureRecoveryRecommendedCount"], 3);
    assert_eq!(
        value["listenerFailureRecoveryOverview"],
        serde_json::json!({
            "schemaVersion": 1,
            "resourceType": "listener",
            "statusEndpoint": "/v1/listener-statuses",
            "status": {
                "severity": "critical",
                "severityLevel": 2,
                "primarySignal": "mixed",
            },
            "severity": "critical",
            "severityLevel": 2,
            "primarySignal": "mixed",
            "drilldown": {
                "filter": "hasEverFailed=true&recoveredFromFailure=false",
                "path": "/v1/listener-statuses?hasEverFailed=true&recoveredFromFailure=false",
                "reason": "inspect listeners that have failed and are not yet recovered",
                "recommendedCount": 3
            },
            "recommendedFilter": "hasEverFailed=true&recoveredFromFailure=false",
            "recommendedPath": "/v1/listener-statuses?hasEverFailed=true&recoveredFromFailure=false",
            "recommendedReason": "inspect listeners that have failed and are not yet recovered",
            "recommendedCount": 3,
            "counts": {
                "recommended": 3,
                "unrecovered": 3,
                "currentSnapshotFailure": 1,
                "historicalFailure": 2
            },
            "unrecoveredCount": 3,
            "currentSnapshotFailureCount": 1,
            "historicalFailureCount": 2
        })
    );
    assert_eq!(
        value["listenerAttentionOverview"],
        serde_json::json!({
            "schemaVersion": 1,
            "resourceType": "listener",
            "statusEndpoint": "/v1/listener-statuses",
            "status": {
                "severity": "critical",
                "severityLevel": 2,
                "primarySignal": "mixed",
            },
            "severity": "critical",
            "severityLevel": 2,
            "primarySignal": "mixed",
            "drilldown": {
                "filter": "attentionRequired=true",
                "path": "/v1/listener-statuses?attentionRequired=true",
                "reason": "inspect listeners currently requiring operator attention across multiple categories",
                "recommendedCount": 3
            },
            "recommendedFilter": "attentionRequired=true",
            "recommendedPath": "/v1/listener-statuses?attentionRequired=true",
            "recommendedReason": "inspect listeners currently requiring operator attention across multiple categories",
            "recommendedCount": 3,
            "counts": {
                "recommended": 3,
                "required": 3,
                "pending": 1,
                "rejected": 1,
                "stale": 1,
                "unrecoveredFailure": 3
            },
            "requiredCount": 3,
            "pendingCount": 1,
            "rejectedCount": 1,
            "staleCount": 1,
            "unrecoveredFailureCount": 3
        })
    );
    assert_eq!(value["listenerOverviews"]["schemaVersion"], 1);
    assert_eq!(
        value["listenerOverviews"]["convergence"],
        value["listenerConvergenceOverview"]
    );
    assert_eq!(
        value["listenerOverviews"]["failureRecovery"],
        value["listenerFailureRecoveryOverview"]
    );
    assert_eq!(
        value["listenerOverviews"]["attention"],
        value["listenerAttentionOverview"]
    );
}
