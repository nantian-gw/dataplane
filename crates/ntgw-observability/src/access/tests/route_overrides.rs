#[test]
fn resolves_route_overrides_from_annotations() {
    let mut annotations = BTreeMap::new();
    annotations.insert(
        "gateway.nantian.dev/access-log-enabled".to_string(),
        "false".to_string(),
    );
    annotations.insert(
        "gateway.nantian.dev/access-log-path".to_string(),
        "/var/log/ntgw/orders.log".to_string(),
    );
    annotations.insert(
        "gateway.nantian.dev/access-log-mode".to_string(),
        "json".to_string(),
    );
    annotations.insert(
        "gateway.nantian.dev/access-log-sample-rate".to_string(),
        "0.25".to_string(),
    );

    let resolved = resolve_access_log_options(&AccessLogOptions::default(), &annotations);
    assert!(!resolved.enabled);
    assert_eq!(resolved.path, "/var/log/ntgw/orders.log");
    assert_eq!(resolved.mode, AccessLogMode::Json);
    assert_eq!(resolved.sample_rate, 0.25);
}

#[test]
fn detects_whether_access_log_can_emit_for_route() {
    let disabled = AccessLogOptions {
        enabled: false,
        ..AccessLogOptions::default()
    };
    assert!(!access_log_enabled_for_route(&disabled, &BTreeMap::new()));

    let mut annotations = BTreeMap::new();
    annotations.insert(
        "gateway.nantian.dev/access-log-enabled".to_string(),
        "true".to_string(),
    );
    assert!(access_log_enabled_for_route(&disabled, &annotations));
}

#[test]
fn resolves_route_overrides_with_custom_annotation_prefix() {
    let base = AccessLogOptions {
        path: "stderr".to_string(),
        format: "%EVENT%".to_string(),
        route_annotation_prefix: "example.com/custom-".to_string(),
        ..AccessLogOptions::default()
    };
    let mut annotations = BTreeMap::new();
    annotations.insert(
        "example.com/custom-enabled".to_string(),
        "false".to_string(),
    );
    annotations.insert("example.com/custom-path".to_string(), "  ".to_string());
    annotations.insert(
        "example.com/custom-format".to_string(),
        "%EVENT% %STATUS%".to_string(),
    );
    annotations.insert(
        "example.com/custom-sample-rate".to_string(),
        "0.75".to_string(),
    );

    let resolved = resolve_access_log_options(&base, &annotations);
    assert!(!resolved.enabled);
    assert_eq!(resolved.path, "stderr");
    assert_eq!(resolved.format, "%EVENT% %STATUS%");
    assert_eq!(resolved.sample_rate, 0.75);
}
