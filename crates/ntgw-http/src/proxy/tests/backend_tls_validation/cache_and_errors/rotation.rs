#[test]
fn build_upstream_peer_reloads_backend_tls_validation_after_snapshot_rotation() {
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
        backend_tls: None,
    };
    let first_snapshot = Snapshot {
        id: "v1".to_string(),
        ..Snapshot::default()
    };
    let second_snapshot = Snapshot {
        id: "v2".to_string(),
        ..Snapshot::default()
    };
    let first_policy = BackendPolicy {
        connect_timeout: None,
        request_timeout: None,
        tls_validation: Some(BackendTlsValidation {
            hostname: "greeter.old.example".to_string(),
            use_system_ca_certificates: false,
            ca_pems: vec![TEST_CLIENT_CERT_PEM.to_string()],
            subject_alt_names: vec![ntgw_ir::BackendSubjectAltName {
                kind: "Hostname".to_string(),
                value: "greeter.old.svc".to_string(),
            }],
            min_version: String::new(),
            max_version: String::new(),
        }),
        session_persistence: None,
        load_balancing: None,
    };
    let second_policy = BackendPolicy {
        connect_timeout: None,
        request_timeout: None,
        tls_validation: Some(BackendTlsValidation {
            hostname: "greeter.new.example".to_string(),
            use_system_ca_certificates: false,
            ca_pems: vec![TEST_SERVER_SAN_CERT_PEM.to_string()],
            subject_alt_names: vec![ntgw_ir::BackendSubjectAltName {
                kind: "Hostname".to_string(),
                value: "greeter.new.svc".to_string(),
            }],
            min_version: String::new(),
            max_version: String::new(),
        }),
        session_persistence: None,
        load_balancing: None,
    };

    let first_peer = build_upstream_peer(
        &first_snapshot,
        &selected,
        Some("GRPCS"),
        Some(&first_policy),
    )
    .expect("first peer");
    let second_peer = build_upstream_peer(
        &second_snapshot,
        &selected,
        Some("GRPCS"),
        Some(&second_policy),
    )
    .expect("second peer");

    assert_eq!(first_peer.sni, "greeter.old.example");
    assert_eq!(second_peer.sni, "greeter.new.example");
    assert!(!first_peer.options.verify_hostname);
    assert!(!second_peer.options.verify_hostname);
    assert!(first_peer
        .options
        .upstream_tls_handshake_complete_hook
        .is_some());
    assert!(second_peer
        .options
        .upstream_tls_handshake_complete_hook
        .is_some());
    assert_ne!(first_peer.group_key, second_peer.group_key);
}
