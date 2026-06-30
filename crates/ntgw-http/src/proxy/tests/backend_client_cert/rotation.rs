#[test]
fn build_upstream_peer_reloads_client_certificate_after_snapshot_rotation() {
    let selected = SelectedBackend { route_policy: None,
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
    let first_snapshot = Snapshot {
        id: "v1".to_string(),
        secrets: vec![ntgw_ir::SecretMaterial {
            namespace: "default".to_string().into(),
            name: "client-cert".to_string().into(),
            cert_pem: TEST_CLIENT_CERT_PEM.to_string(),
            key_pem: TEST_CLIENT_KEY_PEM.to_string(),
        }],
        ..Snapshot::default()
    };
    let second_snapshot = Snapshot {
        id: "v2".to_string(),
        secrets: vec![ntgw_ir::SecretMaterial {
            namespace: "default".to_string().into(),
            name: "client-cert".to_string().into(),
            cert_pem: TEST_SERVER_SAN_CERT_PEM.to_string(),
            key_pem: TEST_SERVER_SAN_KEY_PEM.to_string(),
        }],
        ..Snapshot::default()
    };

    let first_peer =
        build_upstream_peer(&first_snapshot, &selected, Some("HTTPS"), None).expect("peer");
    let second_peer =
        build_upstream_peer(&second_snapshot, &selected, Some("HTTPS"), None).expect("peer");

    let first_cert = first_peer
        .client_cert_key
        .as_ref()
        .expect("first client cert");
    let second_cert = second_peer
        .client_cert_key
        .as_ref()
        .expect("second client cert");
    assert!(!std::sync::Arc::ptr_eq(first_cert, second_cert));
}
