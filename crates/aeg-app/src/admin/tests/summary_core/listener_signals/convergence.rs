fn assert_listener_convergence_summary(value: &serde_json::Value) {
    assert_eq!(value["listenerConvergenceBlockedCount"], 2);
    assert_eq!(
        value["listenerConvergenceBlockedNames"],
        serde_json::json!([])
    );
    assert_eq!(value["listenerConvergenceBlockedHttpCount"], 1);
    assert_eq!(
        value["listenerConvergenceBlockedHttpNames"],
        serde_json::json!([])
    );
    assert_eq!(value["listenerConvergenceBlockedStreamCount"], 1);
    assert_eq!(
        value["listenerConvergenceBlockedStreamNames"],
        serde_json::json!([])
    );
    assert_eq!(value["listenerConvergenceBlockedNoneCount"], 0);
    assert_eq!(
        value["listenerConvergenceBlockedNoneNames"],
        serde_json::json!([])
    );
    assert_eq!(value["listenerConvergenceSeverity"], "warning");
    assert_eq!(value["listenerConvergenceSeverityLevel"], 1);
    assert_eq!(
        value["listenerConvergencePrimarySignal"],
        "awaiting-current-attempt"
    );
    assert_eq!(
        value["listenerConvergenceRecommendedFilter"],
        "attemptProgress=awaiting-current"
    );
    assert_eq!(
        value["listenerConvergenceRecommendedPath"],
        "/v1/listener-statuses?attemptProgress=awaiting-current"
    );
    assert_eq!(
        value["listenerConvergenceRecommendedReason"],
        "inspect listeners that have not yet attempted the active snapshot"
    );
    assert_eq!(value["listenerConvergenceRecommendedCount"], 2);
    assert_eq!(
        value["listenerConvergenceOverview"],
        serde_json::json!({
            "schemaVersion": 1,
            "resourceType": "listener",
            "statusEndpoint": "/v1/listener-statuses",
            "status": {
                "severity": "warning",
                "severityLevel": 1,
                "primarySignal": "awaiting-current-attempt",
            },
            "severity": "warning",
            "severityLevel": 1,
            "primarySignal": "awaiting-current-attempt",
            "drilldown": {
                "filter": "attemptProgress=awaiting-current",
                "path": "/v1/listener-statuses?attemptProgress=awaiting-current",
                "reason": "inspect listeners that have not yet attempted the active snapshot",
                "recommendedCount": 2
            },
            "recommendedFilter": "attemptProgress=awaiting-current",
            "recommendedPath": "/v1/listener-statuses?attemptProgress=awaiting-current",
            "recommendedReason": "inspect listeners that have not yet attempted the active snapshot",
            "recommendedCount": 2,
            "counts": {
                "recommended": 2,
                "blocked": 2,
                "awaitingCurrentAttempt": 2,
                "currentAttemptBlocked": 0,
                "servingDrift": 0
            },
            "blockedCount": 2,
            "awaitingCurrentAttemptCount": 2,
            "currentAttemptBlockedCount": 0,
            "servingDriftCount": 0
        })
    );
    assert_eq!(value["listenerApplyBlockedCount"], 2);
    assert_eq!(value["listenerApplyBlockedNames"], serde_json::json!([]));
    assert_eq!(value["listenerApplyBlockedHttpCount"], 1);
    assert_eq!(
        value["listenerApplyBlockedHttpNames"],
        serde_json::json!([])
    );
    assert_eq!(value["listenerApplyBlockedStreamCount"], 1);
    assert_eq!(
        value["listenerApplyBlockedStreamNames"],
        serde_json::json!([])
    );
    assert_eq!(value["listenerApplyBlockedNoneCount"], 0);
    assert_eq!(
        value["listenerApplyBlockedNoneNames"],
        serde_json::json!([])
    );
    assert_eq!(value["listenerAwaitingCurrentAttemptCount"], 2);
    assert_eq!(
        value["listenerAwaitingCurrentAttemptNames"],
        serde_json::json!([])
    );
    assert_eq!(value["listenerAwaitingCurrentAttemptHttpCount"], 1);
    assert_eq!(
        value["listenerAwaitingCurrentAttemptHttpNames"],
        serde_json::json!([])
    );
    assert_eq!(value["listenerAwaitingCurrentAttemptStreamCount"], 1);
    assert_eq!(
        value["listenerAwaitingCurrentAttemptStreamNames"],
        serde_json::json!([])
    );
    assert_eq!(value["listenerAwaitingCurrentAttemptNoneCount"], 0);
    assert_eq!(
        value["listenerAwaitingCurrentAttemptNoneNames"],
        serde_json::json!([])
    );
    assert_eq!(value["listenerCurrentAttemptBlockedCount"], 0);
    assert_eq!(
        value["listenerCurrentAttemptBlockedNames"],
        serde_json::json!([])
    );
    assert_eq!(value["listenerCurrentAttemptBlockedHttpCount"], 0);
    assert_eq!(
        value["listenerCurrentAttemptBlockedHttpNames"],
        serde_json::json!([])
    );
    assert_eq!(value["listenerCurrentAttemptBlockedStreamCount"], 0);
    assert_eq!(
        value["listenerCurrentAttemptBlockedStreamNames"],
        serde_json::json!([])
    );
    assert_eq!(value["listenerCurrentAttemptBlockedNoneCount"], 0);
    assert_eq!(
        value["listenerCurrentAttemptBlockedNoneNames"],
        serde_json::json!([])
    );
    assert_eq!(value["listenerServingDriftCount"], 0);
    assert_eq!(value["listenerServingDriftNames"], serde_json::json!([]));
    assert_eq!(value["listenerServingDriftHttpCount"], 0);
    assert_eq!(
        value["listenerServingDriftHttpNames"],
        serde_json::json!([])
    );
    assert_eq!(value["listenerServingDriftStreamCount"], 0);
    assert_eq!(
        value["listenerServingDriftStreamNames"],
        serde_json::json!([])
    );
    assert_eq!(value["listenerServingDriftNoneCount"], 0);
    assert_eq!(
        value["listenerServingDriftNoneNames"],
        serde_json::json!([])
    );
    assert_eq!(value["listenerServingCurrentSnapshotCount"], 0);
    assert_eq!(value["listenerServingLastGoodSnapshotCount"], 0);
}
