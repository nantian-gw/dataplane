#[test]
fn rejects_tls_sni_outside_listener_hostname_even_when_route_hostname_matches() {
    let snapshot = Snapshot {
        listeners: vec![Listener {
            name: "default/gw/tls".to_string(),
            address: "0.0.0.0".to_string(),
            addresses: vec!["0.0.0.0".to_string()],
            port: 443,
            protocol: "LISTENER_PROTOCOL_TLS_PASSTHROUGH".to_string(),
            hostnames: vec!["*.example.com".to_string()],
            attached_routes: vec!["default/wildcard-route".to_string()],
            tls: None,
            backend_tls: None,
            metadata: BTreeMap::new(),
        }],
        security_policy: None,
        stream_routes: vec![StreamRoute {
            name: "wildcard-route".to_string(),
            namespace: "default".to_string(),
            kind: "ROUTE_KIND_TLS".to_string(),
            parent_refs: vec![],
            rules: vec![StreamRule {
                name: String::new(),
                matches: vec![StreamMatch {
                    port: 443,
                    sni_hostname: "*.com".to_string(),
                    mode: TlsRouteMode::default(),
                }],
                backend_refs: vec![backend_ref("default", "tls-upstream", 8443)],
            }],
            labels: BTreeMap::new(),
            annotations: BTreeMap::new(),
        }],
        backends: vec![BackendCluster {
            name: "tls-upstream:8443".to_string().into(),
            namespace: "default".to_string().into(),
            protocol: "TCP".to_string().into(),
            endpoints: vec![BackendEndpoint {
                address: "10.0.0.40".to_string(),
                port: 8443,
                healthy: true,
            }],
            wasm_plugin: None,
                ai_service: None,
                token_policy: None,
        
                circuit_breaker: None,}],
        security_policy: None,
        ..Snapshot::default()
    };

    assert!(
        snapshot
            .select_stream_backend("default/gw/tls", Some("non.matching.com"))
            .is_none(),
        "listener hostname should reject SNI outside the listener/route intersection"
    );

    let selected = snapshot
        .select_stream_backend("default/gw/tls", Some("other.example.com"))
        .expect("backend");

    assert_eq!(selected.route_kind, RouteKind::Tls);
    assert_eq!(selected.route_name, "wildcard-route");
    assert_eq!(selected.backend.port, 8443);
}
