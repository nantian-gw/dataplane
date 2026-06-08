fn assert_current_listener_convergence(value: &serde_json::Value) {
    assert_eq!(value["listenerConvergenceBlockedCount"], 2);
    assert_eq!(
        value["listenerConvergenceBlockedNames"],
        serde_json::json!(["stale", "stream-pending"])
    );
    assert_eq!(value["listenerConvergenceBlockedHttpCount"], 1);
    assert_eq!(
        value["listenerConvergenceBlockedHttpNames"],
        serde_json::json!(["stale"])
    );
    assert_eq!(value["listenerConvergenceBlockedStreamCount"], 1);
    assert_eq!(
        value["listenerConvergenceBlockedStreamNames"],
        serde_json::json!(["stream-pending"])
    );
    assert_eq!(value["listenerConvergenceBlockedNoneCount"], 0);
    assert_eq!(
        value["listenerConvergenceBlockedNoneNames"],
        serde_json::json!([])
    );
    assert_eq!(value["listenerConvergenceSeverity"], "warning");
    assert_eq!(value["listenerConvergencePrimarySignal"], "mixed");
    assert_eq!(
        value["listenerConvergenceRecommendedFilter"],
        "attentionRequired=true"
    );
    assert_eq!(
        value["listenerConvergenceRecommendedPath"],
        "/v1/listener-statuses?attentionRequired=true"
    );
    assert_eq!(
        value["listenerConvergenceRecommendedReason"],
        "inspect listeners currently requiring attention across multiple convergence states"
    );
    assert_eq!(value["listenerConvergenceRecommendedCount"], 2);
    assert_eq!(value["listenerApplyBlockedCount"], 1);
    assert_eq!(
        value["listenerApplyBlockedNames"],
        serde_json::json!(["stream-pending"])
    );
    assert_eq!(value["listenerApplyBlockedHttpCount"], 0);
    assert_eq!(
        value["listenerApplyBlockedHttpNames"],
        serde_json::json!([])
    );
    assert_eq!(value["listenerApplyBlockedStreamCount"], 1);
    assert_eq!(
        value["listenerApplyBlockedStreamNames"],
        serde_json::json!(["stream-pending"])
    );
    assert_eq!(value["listenerApplyBlockedNoneCount"], 0);
    assert_eq!(
        value["listenerApplyBlockedNoneNames"],
        serde_json::json!([])
    );
    assert_eq!(value["listenerAwaitingCurrentAttemptCount"], 1);
    assert_eq!(
        value["listenerAwaitingCurrentAttemptNames"],
        serde_json::json!(["stream-pending"])
    );
    assert_eq!(value["listenerAwaitingCurrentAttemptHttpCount"], 0);
    assert_eq!(
        value["listenerAwaitingCurrentAttemptHttpNames"],
        serde_json::json!([])
    );
    assert_eq!(value["listenerAwaitingCurrentAttemptStreamCount"], 1);
    assert_eq!(
        value["listenerAwaitingCurrentAttemptStreamNames"],
        serde_json::json!(["stream-pending"])
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
    assert_eq!(value["listenerServingDriftCount"], 1);
    assert_eq!(
        value["listenerServingDriftNames"],
        serde_json::json!(["stale"])
    );
    assert_eq!(value["listenerServingDriftHttpCount"], 1);
    assert_eq!(
        value["listenerServingDriftHttpNames"],
        serde_json::json!(["stale"])
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
}
