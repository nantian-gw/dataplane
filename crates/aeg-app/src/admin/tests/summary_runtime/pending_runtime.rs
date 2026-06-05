#[test]
fn summary_view_reports_pending_runtime_when_snapshot_is_newer_than_runtime_state() {
    let snapshot = Snapshot {
        id: "v-pending".to_string(),
        listeners: vec![Listener {
            protocol: "LISTENER_PROTOCOL_HTTP".to_string(),
            ..Listener::default()
        }],
        ..Snapshot::default()
    };
    let shared = Snapshot::shared();
    *shared.write() = snapshot;

    let state = build_state_with_parts(
        test_admin_runtime_config(),
        shared,
        RuntimeStats::shared(),
        ClientStats::shared(),
    );

    let value = build_summary_value(&state);
    assert_eq!(value["currentSnapshotStatus"], "accepted");
    assert_eq!(value["currentSnapshotFallbackState"], "none");
    assert_eq!(value["runtimeHttpRequired"], true);
    assert_eq!(value["runtimeHttpCurrentStatus"], "pending");
    assert_eq!(value["runtimeHttpCurrentAccepted"], false);
    assert_eq!(value["runtimeHttpCurrentRejected"], false);
    assert_eq!(value["runtimeTlsRequired"], false);
    assert_eq!(value["runtimeTlsCurrentStatus"], "idle");
    assert_eq!(value["runtimeStreamRequired"], false);
    assert_eq!(value["runtimeStreamCurrentStatus"], "idle");
    assert_eq!(
        value["snapshotOverview"]["summary"],
        serde_json::json!({
            "version": "v-pending",
            "status": "accepted",
            "servingLastGoodSnapshot": false,
            "lastGoodSnapshotVersion": "",
            "fallbackState": "none",
            "rejected": false,
            "rejectionRuntime": "",
        })
    );
    assert_eq!(value["snapshotOverview"]["current"]["status"], "accepted");
    assert_eq!(value["runtimeOverview"]["schemaVersion"], 1);
    assert_eq!(
        value["runtimeOverview"]["summary"],
        serde_json::json!({
            "required": {
                "http": true,
                "tls": false,
                "stream": false,
            },
            "currentStatuses": {
                "http": "pending",
                "tls": "idle",
                "stream": "idle",
            },
            "acceptedPlanes": 0,
            "rejectedPlanes": 0,
        })
    );
    assert_eq!(value["runtimeOverview"]["http"]["required"], true);
    assert_eq!(value["runtimeOverview"]["http"]["status"], "pending");
    assert_eq!(value["runtimeOverview"]["tls"]["required"], false);
    assert_eq!(value["runtimeOverview"]["tls"]["status"], "idle");
    assert_eq!(value["runtimeOverview"]["stream"]["required"], false);
    assert_eq!(value["runtimeOverview"]["stream"]["status"], "idle");
}
