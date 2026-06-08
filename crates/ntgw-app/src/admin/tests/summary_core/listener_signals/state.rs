fn assert_listener_state_summary(value: &serde_json::Value) {
    assert_eq!(value["httpListenerCount"], 1);
    assert_eq!(value["streamListenerCount"], 1);
    assert_eq!(value["listenerCurrentIdleCount"], 0);
    assert_eq!(value["listenerCurrentWarmingCount"], 0);
    assert_eq!(value["listenerCurrentPendingCount"], 2);
    assert_eq!(value["listenerCurrentAcceptedCount"], 0);
    assert_eq!(value["listenerCurrentRetainedCount"], 0);
    assert_eq!(value["listenerCurrentRejectedCount"], 0);
    assert_eq!(value["listenerCurrentStaleCount"], 0);
    assert_eq!(
        value["listenerCurrentPendingNames"],
        serde_json::json!(["", ""])
    );
    assert_eq!(value["listenerCurrentAcceptedNames"], serde_json::json!([]));
    assert_eq!(value["listenerCurrentRetainedNames"], serde_json::json!([]));
    assert_eq!(value["listenerCurrentRejectedNames"], serde_json::json!([]));
    assert_eq!(value["listenerCurrentStaleNames"], serde_json::json!([]));
    assert_eq!(
        value["listenerServingCurrentSnapshotNames"],
        serde_json::json!([])
    );
    assert_eq!(
        value["listenerServingLastGoodSnapshotNames"],
        serde_json::json!([])
    );
    assert_eq!(
        value["listenerServingStateCounts"],
        serde_json::json!({
            "none": 2,
            "currentAccepted": 0,
            "currentRetained": 0,
            "lastGoodRejected": 0,
            "lastGoodStale": 0
        })
    );
    assert_eq!(
        value["listenerRecoveryStateCounts"],
        serde_json::json!({
            "idle": 0,
            "warming": 0,
            "steady": 0,
            "recovered": 0,
            "awaitingCurrent": 2,
            "blockedCurrent": 0,
            "unrecoveredCurrent": 0,
            "unrecoveredHistorical": 0,
            "driftedLastGood": 0
        })
    );
    assert_eq!(
        value["listenerStateOverview"],
        serde_json::json!({
            "schemaVersion": 1,
            "resourceType": "listener",
            "statusEndpoint": "/v1/listener-statuses",
            "current": {
                "idle": 0,
                "warming": 0,
                "pending": 2,
                "accepted": 0,
                "retained": 0,
                "rejected": 0,
                "stale": 0
            },
            "serving": {
                "currentSnapshot": 0,
                "lastGoodSnapshot": 0,
                "drift": 0
            }
        })
    );
}
