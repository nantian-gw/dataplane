fn assert_listener_overviews(value: &serde_json::Value) {
    assert_eq!(value["listenerOverviews"]["schemaVersion"], 1);
    assert_eq!(value["listenerOverviews"]["resourceType"], "listener");
    assert_eq!(
        value["listenerOverviews"]["statusEndpoint"],
        "/v1/listener-statuses"
    );
    assert_eq!(
        value["listenerOverviews"]["overviewKeys"],
        serde_json::json!(["summary", "convergence", "failureRecovery", "attention",])
    );
    assert_eq!(
        value["listenerOverviews"]["summary"],
        serde_json::json!({
            "overviewCount": 3,
            "worstSeverity": "warning",
            "worstSeverityLevel": 1,
            "statuses": {
                "convergence": {
                    "severity": "warning",
                    "severityLevel": 1,
                    "recommendedCount": 2
                },
                "failureRecovery": {
                    "severity": "ok",
                    "severityLevel": 0,
                    "recommendedCount": 0
                },
                "attention": {
                    "severity": "warning",
                    "severityLevel": 1,
                    "recommendedCount": 2
                }
            }
        })
    );
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
