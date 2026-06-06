#[test]
fn summary_current_failures_marks_listener_blocking_and_risk() {
    let value = multiple_current_failures_summary_value();

    assert_eq!(value["listenerCurrentRejectedCount"], 2);
    assert_eq!(value["listenerCurrentPendingCount"], 0);
    assert_eq!(
        value["listenerCurrentRejectedNames"],
        serde_json::json!(["web", "passthrough"])
    );
    assert_eq!(value["listenerRiskPendingUnrecoveredCount"], 0);
    assert_eq!(
        value["listenerRiskPendingUnrecoveredNames"],
        serde_json::json!([])
    );
    assert_eq!(value["listenerApplyBlockedCount"], 2);
    assert_eq!(
        value["listenerApplyBlockedNames"],
        serde_json::json!(["web", "passthrough"])
    );
    assert_eq!(value["listenerApplyBlockedHttpCount"], 1);
    assert_eq!(
        value["listenerApplyBlockedHttpNames"],
        serde_json::json!(["web"])
    );
    assert_eq!(value["listenerApplyBlockedTlsCount"], 1);
    assert_eq!(
        value["listenerApplyBlockedTlsNames"],
        serde_json::json!(["passthrough"])
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
    assert_eq!(value["listenerCurrentAttemptBlockedCount"], 2);
    assert_eq!(
        value["listenerCurrentAttemptBlockedNames"],
        serde_json::json!(["web", "passthrough"])
    );
    assert_eq!(value["listenerCurrentAttemptBlockedHttpCount"], 1);
    assert_eq!(
        value["listenerCurrentAttemptBlockedHttpNames"],
        serde_json::json!(["web"])
    );
    assert_eq!(value["listenerCurrentAttemptBlockedTlsCount"], 1);
    assert_eq!(
        value["listenerCurrentAttemptBlockedTlsNames"],
        serde_json::json!(["passthrough"])
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
    assert_eq!(value["listenerRiskRejectedUnrecoveredCount"], 2);
    assert_eq!(
        value["listenerRiskRejectedUnrecoveredNames"],
        serde_json::json!(["web", "passthrough"])
    );
    assert_eq!(value["listenerRiskStaleUnrecoveredCount"], 0);
    assert_eq!(
        value["listenerRiskStaleUnrecoveredNames"],
        serde_json::json!([])
    );
}
