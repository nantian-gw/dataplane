use super::*;

#[test]
fn build_upstream_peer_rejects_backend_tls_version_bounds() {
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

    let err = build_upstream_peer(
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
                subject_alt_names: Vec::new(),
                min_version: "TLS1_2".to_string(),
                max_version: "TLS1_3".to_string(),
            }),
            session_persistence: None,
            load_balancing: None,
            health_check: None,
            outlier_detection: None,
        }),
    )
    .expect_err("backend TLS version bounds should be rejected");

    assert_eq!(err.etype().as_str(), "UnsupportedBackendTlsValidation");
}
