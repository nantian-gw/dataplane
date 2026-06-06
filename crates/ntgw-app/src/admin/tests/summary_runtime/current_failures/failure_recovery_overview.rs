#[test]
fn summary_current_failures_exposes_failure_recovery_overview() {
    let value = multiple_current_failures_summary_value();

    assert_eq!(value["listenerFailureRecoverySeverity"], "critical");
    assert_eq!(value["listenerFailureRecoverySeverityLevel"], 2);
    assert_eq!(
        value["listenerFailureRecoveryPrimarySignal"],
        "current-snapshot-unrecovered"
    );
    assert_eq!(
        value["listenerFailureRecoveryRecommendedFilter"],
        "unrecoveredFailureAge=current"
    );
    assert_eq!(
        value["listenerFailureRecoveryRecommendedPath"],
        "/v1/listener-statuses?unrecoveredFailureAge=current"
    );
    assert_eq!(
        value["listenerFailureRecoveryRecommendedReason"],
        "inspect listeners whose latest unrecovered failure belongs to the active snapshot"
    );
    assert_eq!(value["listenerFailureRecoveryRecommendedCount"], 2);
    assert_eq!(
        value["listenerFailureRecoveryOverview"],
        serde_json::json!({
            "schemaVersion": 1,
            "resourceType": "listener",
            "statusEndpoint": "/v1/listener-statuses",
            "status": {
                "severity": "critical",
                "severityLevel": 2,
                "primarySignal": "current-snapshot-unrecovered",
            },
            "severity": "critical",
            "severityLevel": 2,
            "primarySignal": "current-snapshot-unrecovered",
            "drilldown": {
                "filter": "unrecoveredFailureAge=current",
                "path": "/v1/listener-statuses?unrecoveredFailureAge=current",
                "reason": "inspect listeners whose latest unrecovered failure belongs to the active snapshot",
                "recommendedCount": 2
            },
            "recommendedFilter": "unrecoveredFailureAge=current",
            "recommendedPath": "/v1/listener-statuses?unrecoveredFailureAge=current",
            "recommendedReason": "inspect listeners whose latest unrecovered failure belongs to the active snapshot",
            "recommendedCount": 2,
            "counts": {
                "recommended": 2,
                "unrecovered": 2,
                "currentSnapshotFailure": 2,
                "historicalFailure": 0
            },
            "unrecoveredCount": 2,
            "currentSnapshotFailureCount": 2,
            "historicalFailureCount": 0
        })
    );
}
