fn assert_listener_convergence(value: &serde_json::Value) {
    assert_eq!(value["listenerConvergenceBlockedCount"], 1);
    assert_eq!(
        value["listenerConvergenceBlockedNames"],
        serde_json::json!(["failed"])
    );
    assert_eq!(value["listenerConvergenceBlockedHttpCount"], 1);
    assert_eq!(
        value["listenerConvergenceBlockedHttpNames"],
        serde_json::json!(["failed"])
    );
    assert_eq!(value["listenerConvergenceBlockedStreamCount"], 0);
    assert_eq!(
        value["listenerConvergenceBlockedStreamNames"],
        serde_json::json!([])
    );
    assert_eq!(value["listenerConvergenceBlockedNoneCount"], 0);
    assert_eq!(
        value["listenerConvergenceBlockedNoneNames"],
        serde_json::json!([])
    );
    assert_eq!(value["listenerConvergenceSeverity"], "critical");
    assert_eq!(value["listenerConvergenceSeverityLevel"], 2);
    assert_eq!(
        value["listenerConvergencePrimarySignal"],
        "current-attempt-blocked"
    );
    assert_eq!(
        value["listenerConvergenceRecommendedFilter"],
        "attemptProgress=blocked-current"
    );
    assert_eq!(
        value["listenerConvergenceRecommendedPath"],
        "/v1/listener-statuses?attemptProgress=blocked-current"
    );
    assert_eq!(
        value["listenerConvergenceRecommendedReason"],
        "inspect listeners that already attempted the active snapshot but remain pending or rejected"
    );
    assert_eq!(value["listenerConvergenceRecommendedCount"], 1);
    assert_eq!(
        value["listenerConvergenceOverview"],
        serde_json::json!({
            "schemaVersion": 1,
            "resourceType": "listener",
            "statusEndpoint": "/v1/listener-statuses",
            "status": {
                "severity": "critical",
                "severityLevel": 2,
                "primarySignal": "current-attempt-blocked",
            },
            "severity": "critical",
            "severityLevel": 2,
            "primarySignal": "current-attempt-blocked",
            "drilldown": {
                "filter": "attemptProgress=blocked-current",
                "path": "/v1/listener-statuses?attemptProgress=blocked-current",
                "reason": "inspect listeners that already attempted the active snapshot but remain pending or rejected",
                "recommendedCount": 1
            },
            "recommendedFilter": "attemptProgress=blocked-current",
            "recommendedPath": "/v1/listener-statuses?attemptProgress=blocked-current",
            "recommendedReason": "inspect listeners that already attempted the active snapshot but remain pending or rejected",
            "recommendedCount": 1,
            "counts": {
                "recommended": 1,
                "blocked": 1,
                "awaitingCurrentAttempt": 0,
                "currentAttemptBlocked": 1,
                "servingDrift": 0
            },
            "blockedCount": 1,
            "awaitingCurrentAttemptCount": 0,
            "currentAttemptBlockedCount": 1,
            "servingDriftCount": 0
        })
    );
    assert_eq!(value["listenerApplyBlockedCount"], 1);
    assert_eq!(
        value["listenerApplyBlockedNames"],
        serde_json::json!(["failed"])
    );
    assert_eq!(value["listenerApplyBlockedHttpCount"], 1);
    assert_eq!(
        value["listenerApplyBlockedHttpNames"],
        serde_json::json!(["failed"])
    );
    assert_eq!(value["listenerApplyBlockedStreamCount"], 0);
    assert_eq!(
        value["listenerApplyBlockedStreamNames"],
        serde_json::json!([])
    );
    assert_eq!(value["listenerApplyBlockedNoneCount"], 0);
    assert_eq!(
        value["listenerApplyBlockedNoneNames"],
        serde_json::json!([])
    );
    assert_eq!(value["listenerAwaitingCurrentAttemptCount"], 0);
    assert_eq!(
        value["listenerAwaitingCurrentAttemptNames"],
        serde_json::json!([])
    );
    assert_eq!(value["listenerCurrentAttemptBlockedCount"], 1);
    assert_eq!(
        value["listenerCurrentAttemptBlockedNames"],
        serde_json::json!(["failed"])
    );
    assert_eq!(value["listenerCurrentAttemptBlockedHttpCount"], 1);
    assert_eq!(
        value["listenerCurrentAttemptBlockedHttpNames"],
        serde_json::json!(["failed"])
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
}
