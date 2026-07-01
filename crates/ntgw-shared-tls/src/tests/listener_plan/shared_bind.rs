#[test]
fn build_listener_plan_keeps_https_terminate_and_tls_passthrough_on_same_bind() {
    let snapshot = Snapshot {
        listeners: vec![
            Listener {
                name: "default/gw/https".to_string().into(),
                address: "0.0.0.0".to_string(),
                port: 443,
                protocol: "LISTENER_PROTOCOL_HTTPS".to_string().into(),
                tls: Some(TlsConfig {
                    enabled: true,
                    passthrough: false,
                    secret_refs: vec!["default/example-cert".to_string()],
                    sni_hosts: vec![],
                    min_version: "1.2".to_string(),
                    max_version: "1.3".to_string(),
                    frontend_validation: None,
                }),
                ..Listener::default()
            },
            Listener {
                name: "default/gw/https-wildcard".to_string().into(),
                address: "0.0.0.0".to_string(),
                port: 443,
                protocol: "LISTENER_PROTOCOL_HTTPS".to_string().into(),
                tls: Some(TlsConfig {
                    enabled: true,
                    passthrough: false,
                    secret_refs: vec!["default/wildcard-cert".to_string()],
                    sni_hosts: vec![],
                    min_version: "1.2".to_string(),
                    max_version: "1.3".to_string(),
                    frontend_validation: None,
                }),
                ..Listener::default()
            },
            Listener {
                name: "default/gw/tls".to_string().into(),
                address: "0.0.0.0".to_string(),
                port: 443,
                protocol: "TLS".to_string().into(),
                tls: Some(TlsConfig {
                    enabled: true,
                    passthrough: true,
                    secret_refs: vec![],
                    sni_hosts: vec![],
                    min_version: String::new(),
                    max_version: String::new(),
                    frontend_validation: None,
                }),
                ..Listener::default()
            },
        ],
        secrets: vec![
            example_secret_material("example-cert"),
            wildcard_secret_material("wildcard-cert"),
        ],
        ..Snapshot::default()
    };

    let plan = build_listener_plan(&snapshot, &RuntimeOptions::default()).expect("plan");
    let shared = plan.binds.get("0.0.0.0:443").expect("shared bind");

    assert!(shared.terminate.is_some(), "terminate surface should exist");
    assert!(
        shared.passthrough.is_some(),
        "passthrough surface should exist"
    );
    assert_eq!(shared.terminate.as_ref().unwrap().listener_names.len(), 2);
    assert_eq!(shared.passthrough.as_ref().unwrap().listener_names.len(), 1);
}
