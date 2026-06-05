fn assert_current_listener_states(value: &serde_json::Value) {
    assert_eq!(value["listenerCurrentIdleCount"], 0);
    assert_eq!(value["listenerCurrentWarmingCount"], 0);
    assert_eq!(value["listenerCurrentPendingCount"], 1);
    assert_eq!(value["listenerCurrentAcceptedCount"], 1);
    assert_eq!(value["listenerCurrentRetainedCount"], 1);
    assert_eq!(value["listenerCurrentRejectedCount"], 0);
    assert_eq!(value["listenerCurrentStaleCount"], 1);
    assert_eq!(
        value["listenerCurrentPendingNames"],
        serde_json::json!(["stream-pending"])
    );
    assert_eq!(
        value["listenerCurrentAcceptedNames"],
        serde_json::json!(["accepted"])
    );
    assert_eq!(
        value["listenerCurrentRetainedNames"],
        serde_json::json!(["retained"])
    );
    assert_eq!(value["listenerCurrentRejectedNames"], serde_json::json!([]));
    assert_eq!(
        value["listenerCurrentStaleNames"],
        serde_json::json!(["stale"])
    );
}
