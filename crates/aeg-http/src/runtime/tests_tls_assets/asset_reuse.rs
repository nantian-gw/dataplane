#[test]
fn materialized_tls_assets_are_reused_for_identical_material() {
    let snapshot = Snapshot {
        listeners: vec![
            Listener {
                name: "default/gw/https-a".to_string(),
                address: "127.0.0.1".to_string(),
                port: 443,
                protocol: "LISTENER_PROTOCOL_HTTPS".to_string(),
                tls: Some(TlsConfig {
                    enabled: true,
                    passthrough: false,
                    secret_refs: vec!["default/example-cert".to_string()],
                    ..TlsConfig::default()
                }),
                ..Listener::default()
            },
            Listener {
                name: "default/gw/https-b".to_string(),
                address: "127.0.0.1".to_string(),
                port: 8443,
                protocol: "LISTENER_PROTOCOL_HTTPS".to_string(),
                tls: Some(TlsConfig {
                    enabled: true,
                    passthrough: false,
                    secret_refs: vec!["default/example-cert".to_string()],
                    ..TlsConfig::default()
                }),
                ..Listener::default()
            },
        ],
        secrets: vec![example_secret_material()],
        ..Snapshot::default()
    };

    let plan = build_listener_plan(
        &snapshot,
        &RuntimeOptions {
            enable_ipv6: false,
            ..RuntimeOptions::default()
        },
        None,
    )
    .expect("plan");

    let asset_dir = super::listener_plan::materialize_tls_assets(&plan).expect("asset dir");
    let file_count = fs::read_dir(&asset_dir).expect("read dir").count();
    fs::remove_dir_all(&asset_dir).expect("cleanup asset dir");

    assert_eq!(file_count, 2);
}

#[test]
fn materialized_tls_assets_are_reused_across_separate_listener_starts() {
    let snapshot = Snapshot {
        listeners: vec![
            Listener {
                name: "default/gw/https-a".to_string(),
                address: "127.0.0.1".to_string(),
                port: 443,
                protocol: "LISTENER_PROTOCOL_HTTPS".to_string(),
                tls: Some(TlsConfig {
                    enabled: true,
                    passthrough: false,
                    secret_refs: vec!["default/example-cert".to_string()],
                    ..TlsConfig::default()
                }),
                ..Listener::default()
            },
            Listener {
                name: "default/gw/https-b".to_string(),
                address: "127.0.0.1".to_string(),
                port: 8443,
                protocol: "LISTENER_PROTOCOL_HTTPS".to_string(),
                tls: Some(TlsConfig {
                    enabled: true,
                    passthrough: false,
                    secret_refs: vec!["default/example-cert".to_string()],
                    ..TlsConfig::default()
                }),
                ..Listener::default()
            },
        ],
        secrets: vec![example_secret_material()],
        ..Snapshot::default()
    };

    let plan = build_listener_plan(
        &snapshot,
        &RuntimeOptions {
            enable_ipv6: false,
            ..RuntimeOptions::default()
        },
        None,
    )
    .expect("plan");
    let first = ListenerPlan {
        listeners: vec![plan.listeners[0].clone()],
    };
    let second = ListenerPlan {
        listeners: vec![plan.listeners[1].clone()],
    };
    let asset_dir = std::env::temp_dir()
        .join("aether-gateway")
        .join("http-listeners")
        .join(super::listener_plan::unique_asset_dir_name());

    let first_write =
        super::listener_plan::materialize_tls_assets_in_dir(&first, &asset_dir).expect("first write");
    let second_write = super::listener_plan::materialize_tls_assets_in_dir(&second, &asset_dir)
        .expect("second write");
    let file_count = fs::read_dir(&asset_dir).expect("read dir").count();
    fs::remove_dir_all(&asset_dir).expect("cleanup asset dir");

    assert_eq!(first_write.reused, 0);
    assert_eq!(second_write.reused, 1);
    assert_eq!(file_count, 2);
}
