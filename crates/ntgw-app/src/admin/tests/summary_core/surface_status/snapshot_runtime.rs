fn assert_snapshot_and_runtime_overviews(value: &serde_json::Value) {
    assert_eq!(value["currentSnapshotStatus"], "rejected");
    assert_eq!(value["currentSnapshotAccepted"], false);
    assert_eq!(value["currentSnapshotRejected"], true);
    assert_eq!(value["servingLastGoodSnapshot"], true);
    assert_eq!(value["currentSnapshotFallbackState"], "last-good-rejected");
    assert_eq!(value["lastGoodSnapshotVersion"], "v1");
    assert_eq!(value["currentSnapshotRejectionVersion"], "v1");
    assert_eq!(value["currentSnapshotRejectionRuntime"], "http+stream");
    assert_eq!(
        value["currentSnapshotRejectionMessage"],
        "HTTP runtime: web: bind conflict; stream runtime: passthrough: tcp bind conflict"
    );
    assert_eq!(value["runtimeHttpRequired"], true);
    assert_eq!(value["runtimeHttpCurrentStatus"], "rejected");
    assert_eq!(value["runtimeHttpCurrentAccepted"], false);
    assert_eq!(value["runtimeHttpCurrentRejected"], true);
    assert_eq!(value["runtimeTlsRequired"], false);
    assert_eq!(value["runtimeTlsCurrentStatus"], "idle");
    assert_eq!(value["runtimeTlsCurrentAccepted"], false);
    assert_eq!(value["runtimeTlsCurrentRejected"], false);
    assert_eq!(value["runtimeStreamRequired"], true);
    assert_eq!(value["runtimeStreamCurrentStatus"], "rejected");
    assert_eq!(value["runtimeStreamCurrentAccepted"], false);
    assert_eq!(value["runtimeStreamCurrentRejected"], true);
    assert_eq!(
        value["snapshotOverview"],
        serde_json::json!({
            "schemaVersion": 1,
            "summary": {
                "version": "v1",
                "status": "rejected",
                "servingLastGoodSnapshot": true,
                "lastGoodSnapshotVersion": "v1",
                "fallbackState": "last-good-rejected",
                "rejected": true,
                "rejectionRuntime": "http+stream",
            },
            "snapshotVersion": "v1",
            "current": {
                "status": "rejected",
                "accepted": false,
                "rejected": true,
            },
            "serving": {
                "lastGoodSnapshot": true,
                "lastGoodSnapshotVersion": "v1",
                "fallbackState": "last-good-rejected",
            },
            "rejection": {
                "version": "v1",
                "runtime": "http+stream",
                "message": "HTTP runtime: web: bind conflict; stream runtime: passthrough: tcp bind conflict",
            }
        })
    );
    assert_eq!(
        value["runtimeOverview"],
        serde_json::json!({
            "schemaVersion": 1,
            "summary": {
                "required": {
                    "http": true,
                    "tls": false,
                    "stream": true,
                },
                "currentStatuses": {
                    "http": "rejected",
                    "tls": "idle",
                    "stream": "rejected",
                },
                "acceptedPlanes": 0,
                "rejectedPlanes": 2,
            },
            "http": {
                "required": true,
                "status": "rejected",
                "accepted": false,
                "rejected": true,
                "stats": {
                    "running": false,
                    "lastExitUnixSeconds": 0,
                    "lastExitMessage": "",
                    "listenerReloadFailures": 1,
                    "lastReloadAttemptVersion": "v1",
                    "lastGoodReloadVersion": "",
                    "lastReloadFailureVersion": "v1",
                    "lastReloadFailureListener": "web",
                    "lastReloadFailureMessage": "bind conflict",
                    "currentFailures": [{ "listener": "web", "message": "bind conflict" }],
                    "tlsAssetReuses": 2,
                }
            },
            "tls": {
                "required": false,
                "status": "idle",
                "accepted": false,
                "rejected": false,
                "stats": {
                    "running": false,
                    "lastExitUnixSeconds": 0,
                    "lastExitMessage": "",
                    "listenerReloadFailures": 0,
                    "lastReloadAttemptVersion": "",
                    "lastGoodReloadVersion": "",
                    "lastReloadFailureVersion": "",
                    "lastReloadFailureListener": "",
                    "lastReloadFailureMessage": "",
                    "currentFailures": [],
                }
            },
            "stream": {
                "required": true,
                "status": "rejected",
                "accepted": false,
                "rejected": true,
                "stats": {
                    "running": false,
                    "lastExitUnixSeconds": 0,
                    "lastExitMessage": "",
                    "listenerReloadFailures": 1,
                    "lastReloadAttemptVersion": "v1",
                    "lastGoodReloadVersion": "",
                    "lastReloadFailureVersion": "v1",
                    "lastReloadFailureListener": "passthrough",
                    "lastReloadFailureMessage": "tcp bind conflict",
                    "currentFailures": [{ "listener": "passthrough", "message": "tcp bind conflict" }],
                }
            }
        })
    );
}
