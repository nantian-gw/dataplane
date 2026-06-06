fn assert_current_listener_serving_and_warnings(value: &serde_json::Value) {
    assert_eq!(value["listenerServingCurrentSnapshotCount"], 2);
    assert_eq!(value["listenerServingLastGoodSnapshotCount"], 1);
    assert_eq!(
        value["listenerServingCurrentSnapshotNames"],
        serde_json::json!(["retained", "accepted"])
    );
    assert_eq!(
        value["listenerServingLastGoodSnapshotNames"],
        serde_json::json!(["stale"])
    );
    assert_eq!(
        value["listenerServingStateCounts"],
        serde_json::json!({
            "none": 1,
            "currentAccepted": 1,
            "currentRetained": 1,
            "lastGoodRejected": 0,
            "lastGoodStale": 1
        })
    );
    assert_eq!(
        value["listenerRecoveryStateCounts"],
        serde_json::json!({
            "idle": 0,
            "warming": 0,
            "steady": 2,
            "recovered": 0,
            "awaitingCurrent": 1,
            "blockedCurrent": 0,
            "unrecoveredCurrent": 0,
            "unrecoveredHistorical": 0,
            "driftedLastGood": 1
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
                "pending": 1,
                "accepted": 1,
                "retained": 1,
                "rejected": 0,
                "stale": 1
            },
            "serving": {
                "currentSnapshot": 2,
                "lastGoodSnapshot": 1,
                "drift": 1
            }
        })
    );
    assert_eq!(
        value["warnings"],
        serde_json::json!([
            "listeners still pending for snapshot v2: stream-pending",
            "listeners still serving last-good snapshot instead of v2: stale",
        ])
    );
}
