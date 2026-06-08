fn assert_current_listener_attention(value: &serde_json::Value) {
    assert_eq!(value["listenerAttentionRequiredCount"], 2);
    assert_eq!(value["listenerAttentionHttpCount"], 1);
    assert_eq!(value["listenerAttentionStreamCount"], 1);
    assert_eq!(value["listenerAttentionNoneCount"], 0);
    assert_eq!(value["listenerAttentionPendingCount"], 1);
    assert_eq!(value["listenerAttentionRejectedCount"], 0);
    assert_eq!(value["listenerAttentionStaleCount"], 1);
    assert_eq!(value["listenerAttentionUnrecoveredFailureCount"], 0);
    assert_eq!(value["listenerAttentionSeverity"], "warning");
    assert_eq!(value["listenerAttentionSeverityLevel"], 1);
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
        value["listenerAttentionRequiredNames"],
        serde_json::json!(["stale", "stream-pending"])
    );
    assert_eq!(
        value["listenerAttentionHttpNames"],
        serde_json::json!(["stale"])
    );
    assert_eq!(
        value["listenerAttentionStreamNames"],
        serde_json::json!(["stream-pending"])
    );
    assert_eq!(value["listenerAttentionNoneNames"], serde_json::json!([]));
    assert_eq!(
        value["listenerAttentionPendingNames"],
        serde_json::json!(["stream-pending"])
    );
    assert_eq!(
        value["listenerAttentionRejectedNames"],
        serde_json::json!([])
    );
    assert_eq!(
        value["listenerAttentionStaleNames"],
        serde_json::json!(["stale"])
    );
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
                "primarySignal": "mixed",
            },
            "severity": "warning",
            "severityLevel": 1,
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
                "pending": 1,
                "rejected": 0,
                "stale": 1,
                "unrecoveredFailure": 0
            },
            "requiredCount": 2,
            "pendingCount": 1,
            "rejectedCount": 0,
            "staleCount": 1,
            "unrecoveredFailureCount": 0
        })
    );
}
