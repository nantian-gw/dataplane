fn assert_listener_recovery_counts(value: &serde_json::Value) {
    assert_eq!(value["listenerHasEverFailedCount"], 2);
    assert_eq!(value["listenerRecoveredFromFailureCount"], 1);
    assert_eq!(value["listenerUnrecoveredFailureCount"], 1);
    assert_eq!(value["listenerRecoveredFromFailureHttpCount"], 1);
    assert_eq!(value["listenerRecoveredFromFailureStreamCount"], 0);
    assert_eq!(value["listenerRecoveredFromFailureNoneCount"], 0);
    assert_eq!(value["listenerUnrecoveredFailureHttpCount"], 1);
    assert_eq!(value["listenerUnrecoveredFailureStreamCount"], 0);
    assert_eq!(value["listenerUnrecoveredFailureNoneCount"], 0);
    assert_eq!(
        value["listenerServingStateCounts"],
        serde_json::json!({
            "none": 1,
            "currentAccepted": 2,
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
            "steady": 1,
            "recovered": 1,
            "awaitingCurrent": 0,
            "blockedCurrent": 0,
            "unrecoveredCurrent": 1,
            "unrecoveredHistorical": 0,
            "driftedLastGood": 0
        })
    );
}
