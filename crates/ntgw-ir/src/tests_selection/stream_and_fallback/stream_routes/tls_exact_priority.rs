#[test]
fn prefers_exact_tls_sni_match_over_wildcard() {
    let snapshot = Snapshot {
        listeners: vec![Listener {
            name: "default/gw/tls".to_string(),
            address: "0.0.0.0".to_string(),
            addresses: vec!["0.0.0.0".to_string()],
            port: 443,
            protocol: "LISTENER_PROTOCOL_TLS_PASSTHROUGH".to_string(),
            hostnames: vec![],
            attached_routes: vec![
                "default/wildcard-route".to_string(),
                "default/exact-route".to_string(),
            ],
            tls: None,
            backend_tls: None,
            metadata: BTreeMap::new(),
        }],
        stream_routes: vec![
            StreamRoute {
                name: "wildcard-route".to_string(),
                namespace: "default".to_string(),
                kind: "ROUTE_KIND_TLS".to_string(),
                parent_refs: vec![],
                rules: vec![StreamRule {
                    name: String::new(),
                    matches: vec![StreamMatch {
                        port: 443,
                        sni_hostname: "*.example.com".to_string(),
                        mode: TlsRouteMode::default(),
                    }],
                    backend_refs: vec![backend_ref("default", "wildcard-upstream", 8443)],
                }],
                labels: BTreeMap::new(),
            annotations: BTreeMap::new(),
            },
            StreamRoute {
                name: "exact-route".to_string(),
                namespace: "default".to_string(),
                kind: "ROUTE_KIND_TLS".to_string(),
                parent_refs: vec![],
                rules: vec![StreamRule {
                    name: String::new(),
                    matches: vec![StreamMatch {
                        port: 443,
                        sni_hostname: "api.example.com".to_string(),
                        mode: TlsRouteMode::default(),
                    }],
                    backend_refs: vec![backend_ref("default", "exact-upstream", 9443)],
                }],
                labels: BTreeMap::new(),
            annotations: BTreeMap::new(),
            },
        ],
        backends: vec![
            BackendCluster {
                name: "wildcard-upstream:8443".to_string().into(),
                namespace: "default".to_string().into(),
                protocol: "TCP".to_string().into(),
                endpoints: vec![BackendEndpoint {
                    address: "10.0.0.41".to_string(),
                    port: 8443,
                    healthy: true,
                }],
                wasm_plugin: None,
                ai_service: None,
                token_policy: None,
            
                circuit_breaker: None,},
            BackendCluster {
                name: "exact-upstream:9443".to_string().into(),
                namespace: "default".to_string().into(),
                protocol: "TCP".to_string().into(),
                endpoints: vec![BackendEndpoint {
                    address: "10.0.0.42".to_string(),
                    port: 9443,
                    healthy: true,
                }],
                wasm_plugin: None,
                ai_service: None,
                token_policy: None,
            
                circuit_breaker: None,},
        ],
        ..Snapshot::default()
    };

    let selected = snapshot
        .select_stream_backend("default/gw/tls", Some("api.example.com"))
        .expect("backend");

    assert_eq!(selected.route_name, "exact-route");
    assert_eq!(selected.backend.address, "10.0.0.42");
    assert_eq!(selected.backend.port, 9443);
}
