#[test]
fn orders_multi_certificate_identities_by_sni_match_then_fallback_order() {
    const CLIENT_CERT_PEM: &str = include_str!("../../../../../testdata/tls/client.crt");
    const CLIENT_KEY_PEM: &str = include_str!("../../../../../testdata/tls/client.key");

    let snapshot = Snapshot {
        listeners: vec![Listener {
            name: "default/gw/https".to_string().into(),
            address: "127.0.0.1".to_string(),
            port: 443,
            protocol: "LISTENER_PROTOCOL_HTTPS".to_string().into(),
            tls: Some(TlsConfig {
                enabled: true,
                passthrough: false,
                secret_refs: vec![
                    "default/client-cert".to_string(),
                    "default/example-cert".to_string(),
                ],
                ..TlsConfig::default()
            }),
            ..Listener::default()
        }],
        secrets: vec![
            SecretMaterial {
                namespace: "default".to_string().into(),
                name: "client-cert".to_string().into(),
                cert_pem: CLIENT_CERT_PEM.to_string(),
                key_pem: CLIENT_KEY_PEM.to_string(),
            },
            example_secret_material(),
        ],
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

    assert_eq!(material.identities.len(), 2);
    assert_eq!(material.identities[0].secret_ref, "default/client-cert");
    assert_eq!(material.identities[1].secret_ref, "default/example-cert");
    assert!(material.identities[1]
        .match_names
        .iter()
        .any(|name| name == "orders.internal.example"));

    let matched = super::ordered_tls_identity_candidates(
        &material.identities,
        Some("orders.internal.example"),
    );
    assert_eq!(matched[0].secret_ref, "default/example-cert");
    assert_eq!(matched[1].secret_ref, "default/client-cert");

    let fallback =
        super::ordered_tls_identity_candidates(&material.identities, Some("unknown.example"));
    assert_eq!(fallback[0].secret_ref, "default/client-cert");
    assert_eq!(fallback[1].secret_ref, "default/example-cert");
}

#[test]
fn wildcard_tls_identity_matches_only_single_subdomain_label() {
    assert!(super::wildcard_hostname_matches(
        "*.example.com",
        "api.example.com"
    ));
    assert!(!super::wildcard_hostname_matches(
        "*.example.com",
        "deep.api.example.com"
    ));
}
