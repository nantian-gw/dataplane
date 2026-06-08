fn assert_listener_failure_recovery_summary(value: &serde_json::Value) {
    assert_eq!(value["listenerHasEverFailedCount"], 0);
    assert_eq!(value["listenerRecoveredFromFailureCount"], 0);
    assert_eq!(value["listenerUnrecoveredFailureCount"], 0);
    assert_eq!(value["listenerRecoveredFromFailureHttpCount"], 0);
    assert_eq!(
        value["listenerRecoveredFromFailureHttpNames"],
        serde_json::json!([])
    );
    assert_eq!(value["listenerRecoveredFromFailureStreamCount"], 0);
    assert_eq!(
        value["listenerRecoveredFromFailureStreamNames"],
        serde_json::json!([])
    );
    assert_eq!(value["listenerRecoveredFromFailureNoneCount"], 0);
    assert_eq!(
        value["listenerRecoveredFromFailureNoneNames"],
        serde_json::json!([])
    );
    assert_eq!(value["listenerUnrecoveredFailureHttpCount"], 0);
    assert_eq!(
        value["listenerUnrecoveredFailureHttpNames"],
        serde_json::json!([])
    );
    assert_eq!(value["listenerUnrecoveredFailureStreamCount"], 0);
    assert_eq!(
        value["listenerUnrecoveredFailureStreamNames"],
        serde_json::json!([])
    );
    assert_eq!(value["listenerUnrecoveredFailureNoneCount"], 0);
    assert_eq!(
        value["listenerUnrecoveredFailureNoneNames"],
        serde_json::json!([])
    );
    assert_eq!(value["listenerFailureRecoverySeverity"], "ok");
    assert_eq!(value["listenerFailureRecoverySeverityLevel"], 0);
    assert_eq!(value["listenerFailureRecoveryPrimarySignal"], "none");
    assert_eq!(value["listenerFailureRecoveryRecommendedFilter"], "");
    assert_eq!(value["listenerFailureRecoveryRecommendedPath"], "");
    assert_eq!(value["listenerFailureRecoveryRecommendedReason"], "");
    assert_eq!(value["listenerFailureRecoveryRecommendedCount"], 0);
    assert_eq!(
        value["listenerFailureRecoveryOverview"],
        serde_json::json!({
            "schemaVersion": 1,
            "resourceType": "listener",
            "statusEndpoint": "/v1/listener-statuses",
            "status": {
                "severity": "ok",
                "severityLevel": 0,
                "primarySignal": "none",
            },
            "severity": "ok",
            "severityLevel": 0,
            "primarySignal": "none",
            "drilldown": {
                "filter": "",
                "path": "",
                "reason": "",
                "recommendedCount": 0
            },
            "recommendedFilter": "",
            "recommendedPath": "",
            "recommendedReason": "",
            "recommendedCount": 0,
            "counts": {
                "recommended": 0,
                "unrecovered": 0,
                "currentSnapshotFailure": 0,
                "historicalFailure": 0
            },
            "unrecoveredCount": 0,
            "currentSnapshotFailureCount": 0,
            "historicalFailureCount": 0
        })
    );
    assert_eq!(value["listenerRiskPendingUnrecoveredCount"], 0);
    assert_eq!(
        value["listenerRiskPendingUnrecoveredNames"],
        serde_json::json!([])
    );
    assert_eq!(value["listenerRiskRejectedUnrecoveredCount"], 0);
    assert_eq!(
        value["listenerRiskRejectedUnrecoveredNames"],
        serde_json::json!([])
    );
    assert_eq!(value["listenerRiskStaleUnrecoveredCount"], 0);
    assert_eq!(
        value["listenerRiskStaleUnrecoveredNames"],
        serde_json::json!([])
    );
    assert_eq!(value["listenerHasEverFailedNames"], serde_json::json!([]));
    assert_eq!(
        value["listenerRecoveredFromFailureNames"],
        serde_json::json!([])
    );
    assert_eq!(
        value["listenerUnrecoveredFailureNames"],
        serde_json::json!([])
    );
}
