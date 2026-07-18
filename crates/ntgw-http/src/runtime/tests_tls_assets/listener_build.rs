#[test]
fn builds_https_listener_from_snapshot_secret() {
    let snapshot = Snapshot {
        listeners: vec![Listener {
            name: "default/gw/https".to_string(),
            address: "0.0.0.0".to_string(),
            port: 443,
            protocol: "LISTENER_PROTOCOL_HTTPS".to_string(),
            hostnames: vec!["example.com".to_string()],
            attached_routes: vec![],
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
        secrets: vec![example_secret_material()],
        ..Snapshot::default()
    };

    let plan = build_listener_plan(&snapshot, &RuntimeOptions::default(), None).expect("plan");
    assert_eq!(plan.listeners.len(), 2);
    assert_eq!(plan.listeners[0].bind, "0.0.0.0:443");
    assert_eq!(plan.listeners[1].bind, "[::]:443");
    assert!(matches!(plan.listeners[0].protocol, ListenerProtocol::Tls(_)));
    assert!(matches!(plan.listeners[1].protocol, ListenerProtocol::Tls(_)));
}

#[test]
fn builds_https_listener_with_frontend_validation_bundle() {
    let snapshot = Snapshot {
        listeners: vec![Listener {
            name: "default/gw/https".to_string(),
            address: "0.0.0.0".to_string(),
            port: 443,
            protocol: "LISTENER_PROTOCOL_HTTPS".to_string(),
            hostnames: vec!["example.com".to_string()],
            attached_routes: vec![],
            tls: Some(TlsConfig {
                enabled: true,
                passthrough: false,
                secret_refs: vec!["default/example-cert".to_string()],
                sni_hosts: vec![],
                min_version: "1.2".to_string(),
                max_version: "1.3".to_string(),
                frontend_validation: Some(ntgw_ir::FrontendValidation {
                    ca_pems: vec!["CA-ONE".to_string(), "CA-TWO".to_string()],
                    mode: String::new(),
                }),
            }),
            ..Listener::default()
        }],
        secrets: vec![example_secret_material()],
        ..Snapshot::default()
    };

    let plan = build_listener_plan(&snapshot, &RuntimeOptions::default(), None).expect("plan");
    let ListenerProtocol::Tls(material) = &plan.listeners[0].protocol else {
        panic!("expected tls listener");
    };
    assert_eq!(
        material.client_ca_bundle_pem.as_deref(),
        Some("CA-ONE\nCA-TWO")
    );
}

#[test]
fn tls_asset_paths_do_not_collide_when_client_ca_bundle_differs() {
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
                    frontend_validation: Some(ntgw_ir::FrontendValidation {
                        ca_pems: vec!["CA-ONE".to_string()],
                        mode: String::new(),
                    }),
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
                    frontend_validation: Some(ntgw_ir::FrontendValidation {
                        ca_pems: vec!["CA-TWO".to_string()],
                        mode: String::new(),
                    }),
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
    let asset_dir = PathBuf::from("/tmp/ntgw-http-runtime-tests");
    let runtime_plan = super::listener_plan::materialize_runtime_plan(&plan, &asset_dir);
    assert_eq!(runtime_plan.listeners.len(), 2);

    let RuntimeListenerProtocol::Tls {
        cert_path: cert_a,
        key_path: key_a,
        client_ca_path: ca_a,
        ..
    } = &runtime_plan.listeners[0].protocol
    else {
        panic!("expected tls runtime listener");
    };
    let RuntimeListenerProtocol::Tls {
        cert_path: cert_b,
        key_path: key_b,
        client_ca_path: ca_b,
        ..
    } = &runtime_plan.listeners[1].protocol
    else {
        panic!("expected tls runtime listener");
    };

    assert_ne!(cert_a, cert_b);
    assert_ne!(key_a, key_b);
    assert_ne!(ca_a, ca_b);
}

#[test]
fn builds_https_listener_with_insecure_frontend_validation_mode() {
    let snapshot = Snapshot {
        listeners: vec![Listener {
            name: "default/gw/https".to_string(),
            address: "127.0.0.1".to_string(),
            port: 443,
            protocol: "LISTENER_PROTOCOL_HTTPS".to_string(),
            tls: Some(TlsConfig {
                enabled: true,
                passthrough: false,
                secret_refs: vec!["default/example-cert".to_string()],
                frontend_validation: Some(ntgw_ir::FrontendValidation {
                    ca_pems: vec!["CA-ONE".to_string()],
                    mode: "AllowInsecureFallback".to_string(),
                }),
                ..TlsConfig::default()
            }),
            ..Listener::default()
        }],
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
    let ListenerProtocol::Tls(material) = &plan.listeners[0].protocol else {
        panic!("expected tls listener");
    };
    assert_eq!(
        material.frontend_validation_mode.as_deref(),
        Some("AllowInsecureFallback")
    );

    let asset_dir = PathBuf::from("/tmp/ntgw-http-runtime-tests");
    let runtime_plan = super::listener_plan::materialize_runtime_plan(&plan, &asset_dir);
    let RuntimeListenerProtocol::Tls {
        frontend_validation_mode,
        ..
    } = &runtime_plan.listeners[0].protocol
    else {
        panic!("expected tls runtime listener");
    };
    assert_eq!(
        frontend_validation_mode.as_deref(),
        Some("AllowInsecureFallback")
    );
}
