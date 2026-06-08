#[test]
fn summary_session_persistence_ephemeral_secret_keeps_snapshot_runtime_surface_ready() {
    let value = ephemeral_session_persistence_summary_value();

    assert_eq!(value["currentSnapshotStatus"], "accepted");
    assert_eq!(value["summarySurface"], "dataplane-summary");
    assert_eq!(value["summarySchemaVersion"], 1);
    assert_eq!(
        value["metaOverview"]["handshake"],
        serde_json::json!({
            "surface": "dataplane-summary",
            "summarySchemaVersion": 1,
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
                "readinessState": "serving-current",
                "version": "v-sticky",
                "status": "accepted",
            },
            "nodeId": "dp-1",
            "cluster": "kind",
            "readinessState": "serving-current",
            "ready": true,
            "snapshotVersion": "v-sticky",
            "snapshotStatus": "accepted",
        })
    );
    assert_eq!(value["currentSnapshotAccepted"], true);
    assert_eq!(value["currentSnapshotRejected"], false);
    assert_eq!(value["servingLastGoodSnapshot"], false);
    assert_eq!(value["currentSnapshotFallbackState"], "none");
    assert_eq!(value["lastGoodSnapshotVersion"], "");
    assert_eq!(value["runtimeHttpRequired"], false);
    assert_eq!(value["runtimeHttpCurrentStatus"], "idle");
    assert_eq!(value["runtimeTlsRequired"], false);
    assert_eq!(value["runtimeTlsCurrentStatus"], "idle");
    assert_eq!(value["runtimeStreamRequired"], false);
    assert_eq!(value["runtimeStreamCurrentStatus"], "idle");
    assert_eq!(
        value["snapshotOverview"],
        serde_json::json!({
            "schemaVersion": 1,
            "summary": {
                "version": "v-sticky",
                "status": "accepted",
                "servingLastGoodSnapshot": false,
                "lastGoodSnapshotVersion": "",
                "fallbackState": "none",
                "rejected": false,
                "rejectionRuntime": "",
            },
            "snapshotVersion": "v-sticky",
            "current": {
                "status": "accepted",
                "accepted": true,
                "rejected": false,
            },
            "serving": {
                "lastGoodSnapshot": false,
                "lastGoodSnapshotVersion": "",
                "fallbackState": "none",
            },
            "rejection": {
                "version": "",
                "runtime": "",
                "message": "",
            }
        })
    );
    assert_eq!(value["runtimeOverview"]["schemaVersion"], 1);
    assert_eq!(
        value["runtimeOverview"]["summary"],
        serde_json::json!({
            "required": {
                "http": false,
                "tls": false,
                "stream": false,
            },
            "currentStatuses": {
                "http": "idle",
                "tls": "idle",
                "stream": "idle",
            },
            "acceptedPlanes": 0,
            "rejectedPlanes": 0,
        })
    );
    assert_eq!(value["runtimeOverview"]["http"]["required"], false);
    assert_eq!(value["runtimeOverview"]["http"]["status"], "idle");
    assert_eq!(value["runtimeOverview"]["tls"]["required"], false);
    assert_eq!(value["runtimeOverview"]["tls"]["status"], "idle");
    assert_eq!(value["runtimeOverview"]["stream"]["required"], false);
    assert_eq!(value["runtimeOverview"]["stream"]["status"], "idle");
    assert_eq!(
        value["healthOverview"],
        serde_json::json!({
            "schemaVersion": 1,
            "status": {
                "ready": true,
                "snapshotStatus": "accepted",
                "readinessState": "serving-current",
                "readinessReason": "current-snapshot-serving",
            },
            "warnings": {
                "count": 1,
                "hasWarnings": true,
                "primaryCategory": "session-persistence-ephemeral-secret",
            },
            "ready": true,
            "readinessState": "serving-current",
            "readinessReason": "current-snapshot-serving",
            "warningCount": 1,
            "hasWarnings": true,
            "primaryWarningCategory": "session-persistence-ephemeral-secret",
            "snapshotStatus": "accepted",
            "runtime": {
                "http": "idle",
                "tls": "idle",
                "stream": "idle",
            }
        })
    );
}
