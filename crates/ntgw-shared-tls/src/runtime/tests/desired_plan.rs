#[test]
fn desired_listener_plan_ignores_https_listener_without_valid_identity() {
    let snapshot = Snapshot {
        listeners: vec![Listener {
            name: "default/gw/https".to_string(),
            address: "0.0.0.0".to_string(),
            port: 443,
            protocol: "LISTENER_PROTOCOL_HTTPS".to_string(),
            tls: Some(TlsConfig {
                enabled: true,
                passthrough: false,
                secret_refs: vec!["default/missing-cert".to_string()],
                sni_hosts: vec![],
                min_version: "1.2".to_string(),
                max_version: "1.3".to_string(),
                frontend_validation: None,
            }),
            ..Listener::default()
        }],
        ..Snapshot::default()
    };

    let desired = desired_listener_plan(&snapshot, &RuntimeOptions::default())
        .expect("invalid https identity should not reject the snapshot");

    assert!(desired.is_none());
}
