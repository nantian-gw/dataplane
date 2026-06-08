#[test]
fn summary_current_failures_exposes_attention_overview() {
    let value = multiple_current_failures_summary_value();

    assert_eq!(value["listenerAttentionSeverity"], "critical");
    assert_eq!(value["listenerAttentionSeverityLevel"], 2);
    assert_eq!(value["listenerAttentionPrimarySignal"], "mixed");
    assert_eq!(
        value["listenerAttentionRecommendedFilter"],
        "attentionRequired=true"
    );
    assert_eq!(
        value["listenerAttentionRecommendedPath"],
        "/v1/listener-statuses?attentionRequired=true"
    );
    assert_eq!(
        value["listenerAttentionRecommendedReason"],
        "inspect listeners currently requiring operator attention across multiple categories"
    );
    assert_eq!(value["listenerAttentionRecommendedCount"], 2);
    assert_eq!(
        value["listenerAttentionOverview"],
        serde_json::json!({
            "schemaVersion": 1,
            "resourceType": "listener",
            "statusEndpoint": "/v1/listener-statuses",
            "status": {
                "severity": "critical",
                "severityLevel": 2,
                "primarySignal": "mixed",
            },
            "severity": "critical",
            "severityLevel": 2,
            "primarySignal": "mixed",
            "drilldown": {
                "filter": "attentionRequired=true",
                "path": "/v1/listener-statuses?attentionRequired=true",
                "reason": "inspect listeners currently requiring operator attention across multiple categories",
                "recommendedCount": 2
            },
            "recommendedFilter": "attentionRequired=true",
            "recommendedPath": "/v1/listener-statuses?attentionRequired=true",
            "recommendedReason": "inspect listeners currently requiring operator attention across multiple categories",
            "recommendedCount": 2,
            "counts": {
                "recommended": 2,
                "required": 2,
                "pending": 0,
                "rejected": 2,
                "stale": 0,
                "unrecoveredFailure": 2
            },
            "requiredCount": 2,
            "pendingCount": 0,
            "rejectedCount": 2,
            "staleCount": 0,
            "unrecoveredFailureCount": 2
        })
    );
}
