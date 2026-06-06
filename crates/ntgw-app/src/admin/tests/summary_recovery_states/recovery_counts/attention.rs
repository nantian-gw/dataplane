fn assert_listener_attention(value: &serde_json::Value) {
    assert_eq!(value["listenerAttentionRequiredCount"], 1);
    assert_eq!(value["listenerAttentionHttpCount"], 1);
    assert_eq!(value["listenerAttentionStreamCount"], 0);
    assert_eq!(value["listenerAttentionNoneCount"], 0);
    assert_eq!(value["listenerAttentionPendingCount"], 1);
    assert_eq!(value["listenerAttentionRejectedCount"], 0);
    assert_eq!(value["listenerAttentionStaleCount"], 0);
    assert_eq!(value["listenerAttentionUnrecoveredFailureCount"], 1);
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
    assert_eq!(value["listenerAttentionRecommendedCount"], 1);
    assert_eq!(
        value["listenerAttentionRequiredNames"],
        serde_json::json!(["failed"])
    );
    assert_eq!(
        value["listenerAttentionHttpNames"],
        serde_json::json!(["failed"])
    );
    assert_eq!(value["listenerAttentionStreamNames"], serde_json::json!([]));
    assert_eq!(value["listenerAttentionNoneNames"], serde_json::json!([]));
    assert_eq!(
        value["listenerAttentionPendingNames"],
        serde_json::json!(["failed"])
    );
    assert_eq!(
        value["listenerAttentionRejectedNames"],
        serde_json::json!([])
    );
    assert_eq!(value["listenerAttentionStaleNames"], serde_json::json!([]));
    assert_eq!(
        value["listenerAttentionUnrecoveredFailureNames"],
        serde_json::json!(["failed"])
    );
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
                "recommendedCount": 1
            },
            "recommendedFilter": "attentionRequired=true",
            "recommendedPath": "/v1/listener-statuses?attentionRequired=true",
            "recommendedReason": "inspect listeners currently requiring operator attention across multiple categories",
            "recommendedCount": 1,
            "counts": {
                "recommended": 1,
                "required": 1,
                "pending": 1,
                "rejected": 0,
                "stale": 0,
                "unrecoveredFailure": 1
            },
            "requiredCount": 1,
            "pendingCount": 1,
            "rejectedCount": 0,
            "staleCount": 0,
            "unrecoveredFailureCount": 1
        })
    );
}
