#[test]
fn build_upstream_peer_rejects_missing_client_certificate_secret() {
    let selected = SelectedBackend {
        route_kind: RouteKind::Grpc,
        route_name: "route".to_string(),
        route_namespace: "default".to_string(),
        rule_index: None,
        route_annotations: BTreeMap::new(),
        listener_name: "default/gw/grpcs".to_string(),
        listener_protocol: "LISTENER_PROTOCOL_HTTPS".to_string(),
        backend: BackendEndpoint {
            address: "127.0.0.1".to_string(),
            port: 9443,
            healthy: true,
        },
        backend_name: "default/greeter:9443".to_string(),
        filters: Vec::new(),
        matched_http_path: None,
        timeouts: None,
        retry: None,
        session_persistence: None,
        backend_tls: Some(BackendTlsConfig {
            client_certificate_ref: "default/missing".to_string(),
        }),
    };

    let err = build_upstream_peer(&Snapshot::default(), &selected, Some("GRPCS"), None)
        .expect_err("missing client certificate secret should fail");

    assert_eq!(err.etype().as_str(), "InvalidBackendClientCertificate");
}
