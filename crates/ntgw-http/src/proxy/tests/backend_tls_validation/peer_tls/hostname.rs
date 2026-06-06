use super::*;

#[test]
fn build_upstream_peer_uses_backend_tls_validation_hostname() {
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
        backend_tls: None,
    };

    let peer = build_upstream_peer(
        &Snapshot::default(),
        &selected,
        Some("HTTPS"),
        Some(&BackendPolicy {
            connect_timeout: None,
            request_timeout: None,
            tls_validation: Some(BackendTlsValidation {
                hostname: "orders.internal.example".to_string(),
                use_system_ca_certificates: true,
                ca_pems: Vec::new(),
                subject_alt_names: Vec::new(),
                min_version: String::new(),
                max_version: String::new(),
            }),
            session_persistence: None,
            load_balancing: None,
        }),
    )
    .expect("peer");

    assert!(peer.is_tls());
    assert_eq!(peer.sni, "orders.internal.example");
    assert!(peer.options.verify_cert);
    assert!(peer.options.verify_hostname);
}
