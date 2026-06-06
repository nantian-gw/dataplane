fn assert_xds_overview(value: &serde_json::Value) {
    assert_eq!(value["xdsConnectFailures"], 1);
    assert_eq!(value["xdsStreamFailures"], 1);
    assert_eq!(
        value["xdsLastConnectError"],
        "dial tcp 127.0.0.1:18080: connection refused"
    );
    assert!(
        value["xdsLastConnectFailureUnixSeconds"]
            .as_u64()
            .expect("xds connect failure timestamp")
            > 0
    );
    assert_eq!(
        value["xdsLastStreamError"],
        "status: Unknown, message: \"h2 protocol error: error reading a body from connection\""
    );
    assert!(
        value["xdsLastStreamFailureUnixSeconds"]
            .as_u64()
            .expect("xds stream failure timestamp")
            > 0
    );
    assert_eq!(value["xdsSnapshotsApplied"], 1);
    assert_eq!(value["xdsSnapshotsNacked"], 1);
    assert_eq!(value["xdsSnapshotsSkipped"], 1);
    assert_eq!(value["xdsLastNackVersion"], "v2");
    assert_eq!(value["xdsLastNackMessage"], "listener reload failed");
    assert_eq!(value["xdsLastSnapshotVersion"], "v1");
    assert_eq!(value["xdsStreamConnected"], false);
    assert_eq!(value["xdsLastControlPlaneContactUnixSeconds"], 0);
    assert_eq!(
        value["xdsOverview"],
        serde_json::json!({
            "schemaVersion": 1,
            "connection": {
                "counts": {
                    "connectFailures": 1,
                    "streamFailures": 1,
                },
                "status": {
                    "streamConnected": false,
                    "lastControlPlaneContactUnixSeconds": 0,
                    "lastConnectError": "dial tcp 127.0.0.1:18080: connection refused",
                    "lastConnectFailureUnixSeconds": value["xdsLastConnectFailureUnixSeconds"],
                    "lastStreamError": "status: Unknown, message: \"h2 protocol error: error reading a body from connection\"",
                    "lastStreamFailureUnixSeconds": value["xdsLastStreamFailureUnixSeconds"],
                },
                "streamConnected": false,
                "lastControlPlaneContactUnixSeconds": 0,
                "connectFailures": 1,
                "streamFailures": 1,
                "lastConnectError": "dial tcp 127.0.0.1:18080: connection refused",
                "lastConnectFailureUnixSeconds": value["xdsLastConnectFailureUnixSeconds"],
                "lastStreamError": "status: Unknown, message: \"h2 protocol error: error reading a body from connection\"",
                "lastStreamFailureUnixSeconds": value["xdsLastStreamFailureUnixSeconds"],
            },
            "connectFailures": 1,
            "streamFailures": 1,
            "streamConnected": false,
            "lastControlPlaneContactUnixSeconds": 0,
            "lastConnectError": "dial tcp 127.0.0.1:18080: connection refused",
            "lastConnectFailureUnixSeconds": value["xdsLastConnectFailureUnixSeconds"],
            "lastStreamError": "status: Unknown, message: \"h2 protocol error: error reading a body from connection\"",
            "lastStreamFailureUnixSeconds": value["xdsLastStreamFailureUnixSeconds"],
            "snapshots": {
                "counts": {
                    "applied": 1,
                    "nacked": 1,
                    "skipped": 1,
                },
                "status": {
                    "lastSnapshotVersion": "v1",
                    "lastNackVersion": "v2",
                    "lastNackMessage": "listener reload failed",
                    "lastApplyUnixSeconds": value["xdsLastApplyUnixSeconds"],
                },
                "applied": 1,
                "nacked": 1,
                "skipped": 1,
                "lastSnapshotVersion": "v1",
                "lastNackVersion": "v2",
                "lastNackMessage": "listener reload failed",
                "lastApplyUnixSeconds": value["xdsLastApplyUnixSeconds"],
            }
        })
    );
}
