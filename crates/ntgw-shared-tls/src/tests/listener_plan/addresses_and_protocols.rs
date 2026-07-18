#[test]
fn build_listener_plan_expands_all_configured_listener_addresses() {
    let snapshot = Snapshot {
        listeners: vec![Listener {
            name: "default/gw/https".to_string(),
            address: String::new(),
            addresses: vec!["192.0.2.10".to_string(), "gw.example.com".to_string()],
            port: 443,
            protocol: "LISTENER_PROTOCOL_HTTPS".to_string(),
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
        }],
        secrets: vec![example_secret_material("example-cert")],
        ..Snapshot::default()
    };

    let plan = build_listener_plan(&snapshot, &RuntimeOptions::default()).expect("plan");
    let binds = plan.binds.keys().cloned().collect::<Vec<_>>();

    assert_eq!(
        binds,
        vec![
            "192.0.2.10:443".to_string(),
            "gw.example.com:443".to_string(),
        ]
    );
}

#[test]
fn build_listener_plan_ignores_non_tls_listeners() {
    let snapshot = Snapshot {
        listeners: vec![
            Listener {
                name: "default/gw/http".to_string(),
                address: "0.0.0.0".to_string(),
                port: 80,
                protocol: "LISTENER_PROTOCOL_HTTP".to_string(),
                ..Listener::default()
            },
            Listener {
                name: "default/gw/tcp".to_string(),
                address: "0.0.0.0".to_string(),
                port: 9000,
                protocol: "LISTENER_PROTOCOL_TCP".to_string(),
                ..Listener::default()
            },
            Listener {
                name: "default/gw/udp".to_string(),
                address: "0.0.0.0".to_string(),
                port: 5300,
                protocol: "LISTENER_PROTOCOL_UDP".to_string(),
                ..Listener::default()
            },
        ],
        ..Snapshot::default()
    };

    let plan = build_listener_plan(&snapshot, &RuntimeOptions::default())
        .expect("non-TLS listeners should produce an empty shared TLS plan");

    assert!(
        plan.binds.is_empty(),
        "non-TLS listeners should not create shared TLS binds"
    );
}

#[test]
fn build_listener_plan_creates_both_surfaces_for_mixed_tls() {
    let snapshot = Snapshot {
        listeners: vec![Listener {
            name: "default/gw/tls-mixed".to_string(),
            address: "0.0.0.0".to_string(),
            port: 443,
            protocol: "LISTENER_PROTOCOL_TLS".to_string(),
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
        }],
        secrets: vec![example_secret_material("example-cert")],
        ..Snapshot::default()
    };

    let plan = build_listener_plan(&snapshot, &RuntimeOptions::default()).expect("plan");
    let shared = plan.binds.get("0.0.0.0:443").expect("shared bind");

    assert!(
        shared.terminate.is_some(),
        "terminate surface should exist for LISTENER_PROTOCOL_TLS (mixed mode)"
    );
    assert!(
        shared.passthrough.is_some(),
        "passthrough surface should exist for LISTENER_PROTOCOL_TLS (mixed mode)"
    );
    assert!(
        shared
            .passthrough
            .as_ref()
            .is_some_and(|p| p.listener_names.contains(&"default/gw/tls-mixed".to_string())),
        "passthrough surface should reference the mixed-mode listener"
    );
    assert!(
        shared
            .terminate
            .as_ref()
            .is_some_and(|t| t.listener_names.contains(&"default/gw/tls-mixed".to_string())),
        "terminate surface should reference the mixed-mode listener"
    );
}
