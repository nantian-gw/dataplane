use super::*;

#[test]
fn build_upstream_peer_uses_post_handshake_subject_alt_name_validation() {
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

    let peer = build_upstream_peer(
        &Snapshot::default(),
        &selected,
        Some("GRPCS"),
        Some(&BackendPolicy {
            connect_timeout: None,
            request_timeout: None,
            tls_validation: Some(BackendTlsValidation {
                hostname: "greeter.internal.example".to_string(),
                use_system_ca_certificates: true,
                ca_pems: Vec::new(),
                subject_alt_names: vec![ntgw_ir::BackendSubjectAltName {
                    kind: "Hostname".to_string(),
                    value: "orders.backend.svc".to_string(),
                }],
                min_version: String::new(),
                max_version: String::new(),
            }),
            session_persistence: None,
            load_balancing: None,
        }),
    )
    .expect("peer");

    assert!(peer.options.verify_cert);
    assert!(!peer.options.verify_hostname);
    assert!(peer.options.alternative_cn.is_none());
    assert!(peer.options.upstream_tls_handshake_complete_hook.is_some());
    assert_ne!(peer.group_key, 0);
}
