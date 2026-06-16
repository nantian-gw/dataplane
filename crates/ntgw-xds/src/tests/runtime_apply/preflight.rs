#[test]
fn preflight_rejection_keeps_last_good_snapshot() {
    let snapshot = Snapshot::shared();
    *snapshot.write() = Snapshot {
        id: "last-good-v1".to_string(),
        ..Snapshot::default()
    };

    let stats = ClientStats::shared();
    stats.observe_snapshot_applied("last-good-v1");

    let config = ConfigSnapshot {
        id: "rejected-v2".to_string(),
        compatibility_profile: "gateway.control.v1.dataplane".to_string(),
        required_features: vec![
            "route.labels.v1".to_string(),
            "backend.wasm_plugin.v1".to_string(),
        ],
        ..ConfigSnapshot::default()
    };

    let supported = canonicalize_supported_features(["core.v1"]);
    let result = preflight_snapshot_before_swap(&config, None, &supported, &stats);

    assert_eq!(
        result,
        Err((
            "rejected-v2".to_string(),
            "snapshot requires unsupported features: backend.wasm_plugin.v1, route.labels.v1"
                .to_string(),
        ))
    );
    assert_eq!(snapshot.read().id, "last-good-v1");

    let stats = stats.snapshot();
    assert_eq!(stats.snapshots_applied, 1);
    assert_eq!(stats.snapshots_nacked, 1);
    assert_eq!(stats.last_snapshot_version, "last-good-v1");
    assert_eq!(stats.last_nack_version, "rejected-v2");
    assert_eq!(
        stats.last_nack_message,
        "snapshot requires unsupported features: backend.wasm_plugin.v1, route.labels.v1"
    );
}
