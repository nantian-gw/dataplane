#[test]
fn skips_malformed_tls_secret_and_keeps_later_valid_listener_on_same_bind() {
    let snapshot = Snapshot {
        listeners: vec![
            Listener {
                name: "default/gw/bad-https".to_string(),
                address: "0.0.0.0".to_string(),
                port: 443,
                protocol: "LISTENER_PROTOCOL_HTTPS".to_string(),
                tls: Some(TlsConfig {
                    enabled: true,
                    passthrough: false,
                    secret_refs: vec!["default/bad-cert".to_string()],
                    sni_hosts: vec![],
                    min_version: "1.2".to_string(),
                    max_version: "1.3".to_string(),
                    frontend_validation: None,
                }),
                ..Listener::default()
            },
            Listener {
                name: "default/gw/good-https".to_string(),
                address: "0.0.0.0".to_string(),
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
            },
        ],
        secrets: vec![
            SecretMaterial {
                namespace: "default".to_string(),
                name: "bad-cert".to_string(),
                cert_pem: "-----BEGIN CERTIFICATE-----\nmalformed\n-----END CERTIFICATE-----\n"
                    .to_string(),
                key_pem: VALID_SERVER_KEY_PEM.to_string(),
            htpasswd: String::new(),
                        oidc_client_secret: String::new(),
            },
            example_secret_material(),
        ],
        ..Snapshot::default()
    };

    let plan = build_listener_plan(&snapshot, &RuntimeOptions::default(), None).expect("plan");
    assert_eq!(plan.listeners.len(), 2);
    assert_eq!(plan.listeners[0].bind, "0.0.0.0:443");
    assert_eq!(plan.listeners[1].bind, "[::]:443");
    assert_eq!(plan.listeners[0].name, "default/gw/good-https");
    assert_eq!(plan.listeners[1].name, "default/gw/good-https");
    assert!(matches!(plan.listeners[0].protocol, ListenerProtocol::Tls(_)));
}

#[test]
fn skips_https_listener_when_tls_passthrough_uses_same_bind() {
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
                name: "default/gw/https".to_string(),
                address: "0.0.0.0".to_string(),
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
            },
            Listener {
                name: "default/gw/tls".to_string(),
                address: "0.0.0.0".to_string(),
                port: 443,
                protocol: "TLS".to_string(),
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
        secrets: vec![example_secret_material()],
        ..Snapshot::default()
    };

    let plan = build_listener_plan(&snapshot, &RuntimeOptions::default(), None).expect("plan");
    assert_eq!(plan.listeners.len(), 2);
    assert_eq!(plan.listeners[0].bind, "0.0.0.0:80");
    assert_eq!(plan.listeners[1].bind, "[::]:80");
    assert!(matches!(plan.listeners[0].protocol, ListenerProtocol::Plain));
    assert!(matches!(plan.listeners[1].protocol, ListenerProtocol::Plain));
}

#[test]
fn runtime_ignores_https_listener_bind_that_shared_tls_owns() {
    let snapshot = Snapshot {
        id: "v2".to_string(),
        listeners: vec![Listener {
            name: "default/gw/https".to_string(),
            address: "127.0.0.1".to_string(),
            addresses: vec!["127.0.0.1".to_string()],
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
        secrets: vec![example_secret_material()],
        ..Snapshot::default()
    };
    let runtime_stats = ntgw_observability::RuntimeStats::shared();
    runtime_stats.observe_stream_listener_reload_success("v1");

    let result = build_listener_plan_with_bind_checker_for_runtime(
        &snapshot,
        &RuntimeOptions {
            enable_ipv6: false,
            ..RuntimeOptions::default()
        },
        &Default::default(),
        |_| {
            Err(std::io::Error::new(
                std::io::ErrorKind::AddrInUse,
                "Address already in use",
            ))
        },
        &runtime_stats.snapshot(),
    );

    assert!(result.plan.is_none());
    assert!(!result.retry_start);
    assert!(result.deferred_binds.is_empty());
}
