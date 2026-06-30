use super::*;

#[test]
fn build_upstream_peer_accepts_custom_backend_tls_validation() {
    let selected = SelectedBackend {
        route_policy: None,
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
                use_system_ca_certificates: false,
                ca_pems: vec![TEST_CLIENT_CERT_PEM.to_string()],
                subject_alt_names: Vec::new(),
                min_version: String::new(),
                max_version: String::new(),
            }),
            session_persistence: None,
            load_balancing: None,
        }),
    )
    .expect("custom CA validation should be accepted");

    assert!(peer.options.ca.is_some());
    assert!(peer.options.verify_cert);
    assert!(peer.options.verify_hostname);
}
