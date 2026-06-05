#[test]
fn build_listener_plan_allows_frontend_validation_with_unvalidated_listener_on_same_bind() {
    let snapshot = Snapshot {
        listeners: vec![
            Listener {
                name: "default/gw/https-strict".to_string(),
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
                    frontend_validation: Some(FrontendValidation {
                        ca_pems: vec!["strict-ca".to_string()],
                        mode: "RequireClientCertificate".to_string(),
                    }),
                }),
                ..Listener::default()
            },
            Listener {
                name: "default/gw/https-unvalidated".to_string(),
                address: "0.0.0.0".to_string(),
                port: 443,
                protocol: "LISTENER_PROTOCOL_HTTPS".to_string(),
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
        ],
        secrets: vec![
            example_secret_material("example-cert"),
            wildcard_secret_material("wildcard-cert"),
        ],
        ..Snapshot::default()
    };

    let plan = build_listener_plan(&snapshot, &RuntimeOptions::default()).expect("plan");
    let terminate = plan
        .binds
        .get("0.0.0.0:443")
        .and_then(|bind| bind.terminate.as_ref())
        .expect("terminate surface");

    assert_eq!(
        terminate.listener_names,
        vec![
            "default/gw/https-strict".to_string(),
            "default/gw/https-unvalidated".to_string(),
        ]
    );
    assert_eq!(
        terminate.frontend_validation_mode.as_deref(),
        Some("RequireClientCertificate")
    );
    assert_eq!(terminate.client_ca_bundle_pem.as_deref(), Some("strict-ca"));
}

#[test]
fn build_listener_plan_rejects_incompatible_frontend_validation_on_same_bind() {
    let snapshot = Snapshot {
        listeners: vec![
            Listener {
                name: "default/gw/https-strict".to_string(),
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
                    frontend_validation: Some(FrontendValidation {
                        ca_pems: vec!["strict-ca".to_string()],
                        mode: "RequireClientCertificate".to_string(),
                    }),
                }),
                ..Listener::default()
            },
            Listener {
                name: "default/gw/https-fallback".to_string(),
                address: "0.0.0.0".to_string(),
                port: 443,
                protocol: "LISTENER_PROTOCOL_HTTPS".to_string(),
                tls: Some(TlsConfig {
                    enabled: true,
                    passthrough: false,
                    secret_refs: vec!["default/wildcard-cert".to_string()],
                    sni_hosts: vec![],
                    min_version: "1.2".to_string(),
                    max_version: "1.3".to_string(),
                    frontend_validation: Some(FrontendValidation {
                        ca_pems: vec!["fallback-ca".to_string()],
                        mode: "AllowInsecureFallback".to_string(),
                    }),
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

    let err = build_listener_plan(&snapshot, &RuntimeOptions::default())
        .expect_err("conflicting frontend validation must fail");

    assert!(err.to_string().contains("frontend validation"));
    assert!(err.to_string().contains("0.0.0.0:443"));
}
