#[test]
fn snapshot_version_prefers_payload_id() {
    let snapshot = ConfigSnapshot {
        id: "payload-v2".to_string(),
        ..ConfigSnapshot::default()
    };

    let version = snapshot_version_from_response("message-v1", &snapshot).expect("version");

    assert_eq!(version, "payload-v2");
}

#[test]
fn snapshot_without_version_is_treated_as_apply_required() {
    assert!(should_apply_snapshot("v1", None));
    assert!(!should_apply_snapshot("v1", Some("v1")));
    assert!(should_apply_snapshot("v1", Some("v2")));
}
