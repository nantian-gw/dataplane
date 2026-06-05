#[test]
fn build_upstream_peer_uses_client_certificate_for_tls_backends() {
    let snapshot = Snapshot {
        secrets: vec![aeg_ir::SecretMaterial {
            namespace: "default".to_string(),
            name: "client-cert".to_string(),
            cert_pem: TEST_CLIENT_CERT_PEM.to_string(),
            key_pem: TEST_CLIENT_KEY_PEM.to_string(),
        }],
        ..Snapshot::default()
    };
    let selected = SelectedBackend {
        route_kind: RouteKind::Http,
        route_name: "route".to_string(),
        route_namespace: "default".to_string(),
        rule_index: None,
        route_annotations: BTreeMap::new(),
        listener_name: "default/gw/https".to_string(),
        listener_protocol: "LISTENER_PROTOCOL_HTTPS".to_string(),
        backend: BackendEndpoint {
            address: "127.0.0.1".to_string(),
            port: 8443,
            healthy: true,
        },
        backend_name: "default/echo:8443".to_string(),
        filters: Vec::new(),
        matched_http_path: None,
        timeouts: None,
        retry: None,
        session_persistence: None,
        backend_tls: Some(BackendTlsConfig {
            client_certificate_ref: "default/client-cert".to_string(),
        }),
    };

    let peer = build_upstream_peer(&snapshot, &selected, Some("HTTPS"), None).expect("peer");

    assert!(peer.is_tls());
    assert_eq!(peer.sni, "echo.default.svc");
    assert!(peer.client_cert_key.is_some());
}
