fn assert_listener_attention_summary(value: &serde_json::Value) {
    assert_eq!(value["listenerAttentionRequiredCount"], 2);
    assert_eq!(
        value["listenerAttentionRequiredNames"],
        serde_json::json!([])
    );
    assert_eq!(value["listenerAttentionHttpCount"], 1);
    assert_eq!(value["listenerAttentionHttpNames"], serde_json::json!([]));
    assert_eq!(value["listenerAttentionStreamCount"], 1);
    assert_eq!(value["listenerAttentionStreamNames"], serde_json::json!([]));
    assert_eq!(value["listenerAttentionNoneCount"], 0);
    assert_eq!(value["listenerAttentionNoneNames"], serde_json::json!([]));
    assert_eq!(value["listenerAttentionPendingCount"], 2);
    assert_eq!(
        value["listenerAttentionPendingNames"],
        serde_json::json!([])
    );
    assert_eq!(value["listenerAttentionRejectedCount"], 0);
    assert_eq!(
        value["listenerAttentionRejectedNames"],
        serde_json::json!([])
    );
    assert_eq!(value["listenerAttentionStaleCount"], 0);
    assert_eq!(value["listenerAttentionStaleNames"], serde_json::json!([]));
    assert_eq!(value["listenerAttentionUnrecoveredFailureCount"], 0);
    assert_eq!(value["listenerAttentionSeverity"], "warning");
    assert_eq!(value["listenerAttentionSeverityLevel"], 1);
    assert_eq!(value["listenerAttentionPrimarySignal"], "pending");
    assert_eq!(
        value["listenerAttentionRecommendedFilter"],
        "attentionReason=pending"
    );
    assert_eq!(
        value["listenerAttentionRecommendedPath"],
        "/v1/listener-statuses?attentionReason=pending"
    );
    assert_eq!(
        value["listenerAttentionRecommendedReason"],
        "inspect listeners currently marked pending"
    );
    assert_eq!(value["listenerAttentionRecommendedCount"], 2);
    assert_eq!(
        value["listenerAttentionUnrecoveredFailureNames"],
        serde_json::json!([])
    );
    assert_eq!(
        value["listenerAttentionOverview"],
        serde_json::json!({
            "schemaVersion": 1,
            "resourceType": "listener",
            "statusEndpoint": "/v1/listener-statuses",
            "status": {
                "severity": "warning",
                "severityLevel": 1,
                "primarySignal": "pending",
            },
            "severity": "warning",
            "severityLevel": 1,
            "primarySignal": "pending",
            "drilldown": {
                "filter": "attentionReason=pending",
                "path": "/v1/listener-statuses?attentionReason=pending",
                "reason": "inspect listeners currently marked pending",
                "recommendedCount": 2
            },
            "recommendedFilter": "attentionReason=pending",
            "recommendedPath": "/v1/listener-statuses?attentionReason=pending",
            "recommendedReason": "inspect listeners currently marked pending",
            "recommendedCount": 2,
            "counts": {
                "recommended": 2,
                "required": 2,
                "pending": 2,
                "rejected": 0,
                "stale": 0,
                "unrecoveredFailure": 0
            },
            "requiredCount": 2,
            "pendingCount": 2,
            "rejectedCount": 0,
            "staleCount": 0,
            "unrecoveredFailureCount": 0
        })
    );
}
