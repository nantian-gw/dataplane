fn assert_listener_overviews_and_warnings(value: &serde_json::Value) {
    assert_eq!(value["listenerOverviews"]["schemaVersion"], 1);
    assert_eq!(
        value["listenerOverviews"]["summary"],
        serde_json::json!({
            "overviewCount": 3,
            "worstSeverity": "critical",
            "worstSeverityLevel": 2,
            "statuses": {
                "convergence": {
                    "severity": "critical",
                    "severityLevel": 2,
                    "recommendedCount": 1
                },
                "failureRecovery": {
                    "severity": "critical",
                    "severityLevel": 2,
                    "recommendedCount": 1
                },
                "attention": {
                    "severity": "critical",
                    "severityLevel": 2,
                    "recommendedCount": 1
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
    assert_eq!(
        value["warnings"],
        serde_json::json!([
            "listeners still pending for snapshot v2: failed",
            "listeners with observed failures not yet recovered: failed"
        ])
    );
}
