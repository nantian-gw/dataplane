#[test]
fn build_upstream_peer_reuses_backend_tls_validation_cache_for_equivalent_policy_content() {
    let selected = SelectedBackend { route_policy: None,
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
        id: "v-cache".to_string(),
        ..Snapshot::default()
    };
    let second_snapshot = Snapshot {
        id: "v-cache".to_string(),
        ..Snapshot::default()
    };
    let first_policy = BackendPolicy {
        connect_timeout: None,
        request_timeout: None,
        tls_validation: Some(BackendTlsValidation {
            hostname: "greeter.cache.example".to_string(),
            use_system_ca_certificates: false,
            ca_pems: vec![TEST_CLIENT_CERT_PEM.to_string()],
            subject_alt_names: vec![ntgw_ir::BackendSubjectAltName {
                kind: "Hostname".to_string(),
                value: "greeter.cache.svc".to_string(),
            }],
            min_version: String::new(),
            max_version: String::new(),
        }),
        session_persistence: None,
        load_balancing: None,
        health_check: None,
        outlier_detection: None,
    };
    let second_policy = BackendPolicy {
        connect_timeout: None,
        request_timeout: None,
        tls_validation: Some(BackendTlsValidation {
            hostname: "greeter.cache.example".to_string(),
            use_system_ca_certificates: false,
            ca_pems: vec![TEST_CLIENT_CERT_PEM.to_string()],
            subject_alt_names: vec![ntgw_ir::BackendSubjectAltName {
                kind: "Hostname".to_string(),
                value: "greeter.cache.svc".to_string(),
            }],
            min_version: String::new(),
            max_version: String::new(),
        }),
        session_persistence: None,
        load_balancing: None,
        health_check: None,
        outlier_detection: None,
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

    let first_ca = first_peer.options.ca.as_ref().expect("first ca bundle");
    let second_ca = second_peer.options.ca.as_ref().expect("second ca bundle");
    assert!(std::sync::Arc::ptr_eq(first_ca, second_ca));
    assert_eq!(first_peer.group_key, second_peer.group_key);
    assert!(first_peer
        .options
        .upstream_tls_handshake_complete_hook
        .is_some());
    assert!(second_peer
        .options
        .upstream_tls_handshake_complete_hook
        .is_some());
}

#[test]
fn build_upstream_peer_reuses_backend_tls_validation_cache_across_interleaved_snapshots() {
    let selected = SelectedBackend { route_policy: None,
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
        id: "v-cache-policy-1".to_string(),
        ..Snapshot::default()
    };
    let interleaved_snapshot = Snapshot {
        id: "v-cache-policy-2".to_string(),
        ..Snapshot::default()
    };
    let second_snapshot = Snapshot {
        id: "v-cache-policy-1".to_string(),
        ..Snapshot::default()
    };
    let cached_policy = BackendPolicy {
        connect_timeout: None,
        request_timeout: None,
        tls_validation: Some(BackendTlsValidation {
            hostname: "greeter.cache.example".to_string(),
            use_system_ca_certificates: false,
            ca_pems: vec![TEST_CLIENT_CERT_PEM.to_string()],
            subject_alt_names: vec![ntgw_ir::BackendSubjectAltName {
                kind: "Hostname".to_string(),
                value: "greeter.cache.svc".to_string(),
            }],
            min_version: String::new(),
            max_version: String::new(),
        }),
        session_persistence: None,
        load_balancing: None,
        health_check: None,
        outlier_detection: None,
    };
    let interleaved_policy = BackendPolicy {
        connect_timeout: None,
        request_timeout: None,
        tls_validation: Some(BackendTlsValidation {
            hostname: "greeter.rotate.example".to_string(),
            use_system_ca_certificates: false,
            ca_pems: vec![TEST_SERVER_SAN_CERT_PEM.to_string()],
            subject_alt_names: vec![ntgw_ir::BackendSubjectAltName {
                kind: "Hostname".to_string(),
                value: "greeter.rotate.svc".to_string(),
            }],
            min_version: String::new(),
            max_version: String::new(),
        }),
        session_persistence: None,
        load_balancing: None,
        health_check: None,
        outlier_detection: None,
    };

    let first_peer = build_upstream_peer(
        &first_snapshot,
        &selected,
        Some("GRPCS"),
        Some(&cached_policy),
    )
    .expect("first peer");
    build_upstream_peer(
        &interleaved_snapshot,
        &selected,
        Some("GRPCS"),
        Some(&interleaved_policy),
    )
    .expect("interleaved peer");
    let second_peer = build_upstream_peer(
        &second_snapshot,
        &selected,
        Some("GRPCS"),
        Some(&cached_policy),
    )
    .expect("second peer");

    let first_ca = first_peer.options.ca.as_ref().expect("first ca bundle");
    let second_ca = second_peer.options.ca.as_ref().expect("second ca bundle");
    assert!(std::sync::Arc::ptr_eq(first_ca, second_ca));
    assert_eq!(first_peer.group_key, second_peer.group_key);
    assert!(first_peer
        .options
        .upstream_tls_handshake_complete_hook
        .is_some());
    assert!(second_peer
        .options
        .upstream_tls_handshake_complete_hook
        .is_some());
}
