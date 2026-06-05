fn assert_meta_health_and_warning_overviews(value: &serde_json::Value) {
    assert_eq!(value["ready"], true);
    assert_eq!(value["summarySurface"], "dataplane-summary");
    assert_eq!(value["summarySchemaVersion"], 1);
    assert_eq!(
        value["metaOverview"],
        serde_json::json!({
            "schemaVersion": 1,
            "surface": "dataplane-summary",
            "handshake": {
                "surface": "dataplane-summary",
                "summarySchemaVersion": 1,
            },
            "overviewKeys": [
                "meta",
                "instance",
                "health",
                "warnings",
                "snapshot",
                "runtime",
                "resources",
                "features",
                "xds",
                "traffic",
                "overload",
                "listenerState",
                "listenerSignals",
            ],
            "overviewSchemas": {
                "summaryOverviews": 1,
                "instance": 1,
                "health": 1,
                "warnings": 1,
                "snapshot": 1,
                "runtime": 1,
                "resources": 1,
                "features": 1,
                "xds": 1,
                "traffic": 1,
                "overload": 1,
                "listenerState": 1,
                "listenerSignals": 1,
            }
        })
    );
    assert_eq!(
        value["instanceOverview"],
        serde_json::json!({
            "schemaVersion": 1,
            "identity": {
                "nodeId": "dp-1",
                "cluster": "kind",
            },
            "snapshot": {
                "ready": true,
                "readinessState": "serving-last-good",
                "version": "v1",
                "status": "rejected",
            },
            "nodeId": "dp-1",
            "cluster": "kind",
            "readinessState": "serving-last-good",
            "ready": true,
            "snapshotVersion": "v1",
            "snapshotStatus": "rejected",
        })
    );
    assert_eq!(
        value["healthOverview"],
        serde_json::json!({
            "schemaVersion": 1,
            "status": {
                "ready": true,
                "snapshotStatus": "rejected",
                "readinessState": "serving-last-good",
                "readinessReason": "serving-last-good-after-rejection",
            },
            "warnings": {
                "count": 2,
                "hasWarnings": true,
                "primaryCategory": "runtime-http-reload-failure",
            },
            "ready": true,
            "readinessState": "serving-last-good",
            "readinessReason": "serving-last-good-after-rejection",
            "warningCount": 2,
            "hasWarnings": true,
            "primaryWarningCategory": "runtime-http-reload-failure",
            "snapshotStatus": "rejected",
            "runtime": {
                "http": "rejected",
                "tls": "idle",
                "stream": "rejected",
            }
        })
    );
    assert_eq!(
        value["warningOverview"],
        serde_json::json!({
            "schemaVersion": 1,
            "status": {
                "count": 2,
                "hasWarnings": true,
                "primaryCategory": "runtime-http-reload-failure",
                "primaryMessage": "latest HTTP listener reload for snapshot v1 failed: web: bind conflict"
            },
            "count": 2,
            "hasWarnings": true,
            "categories": [
                "runtime-http-reload-failure",
                "runtime-stream-reload-failure"
            ],
            "counts": {
                "sessionPersistenceEphemeralSecret": 0,
                "runtimeHttpReloadFailure": 1,
                "runtimeTlsReloadFailure": 0,
                "runtimeStreamReloadFailure": 1,
                "listenerPending": 0,
                "listenerRejected": 0,
                "listenerStale": 0,
                "listenerUnrecoveredFailure": 0
            },
            "messages": [
                "latest HTTP listener reload for snapshot v1 failed: web: bind conflict",
                "latest stream listener reload for snapshot v1 failed: passthrough: tcp bind conflict"
            ]
        })
    );
    assert_eq!(
        value["warningCategories"],
        serde_json::json!([
            "runtime-http-reload-failure",
            "runtime-stream-reload-failure"
        ])
    );
    assert_eq!(
        value["warningCategoryCounts"],
        serde_json::json!({
            "sessionPersistenceEphemeralSecret": 0,
            "runtimeHttpReloadFailure": 1,
            "runtimeTlsReloadFailure": 0,
            "runtimeStreamReloadFailure": 1,
            "listenerPending": 0,
            "listenerRejected": 0,
            "listenerStale": 0,
            "listenerUnrecoveredFailure": 0
        })
    );
}
