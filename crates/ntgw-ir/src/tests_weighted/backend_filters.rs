#[test]
fn select_http_backend_merges_backend_ref_filters() {
    let snapshot = Snapshot {
        http_routes: vec![HttpRoute {
            name: "headers".to_string().into(),
            namespace: "default".to_string().into(),
            hostnames: vec!["api.example.com".to_string()],
            parent_refs: vec![],
            rules: vec![HttpRule {
                name: String::new(),
                matches: vec![],
                filters: vec![Filter {
                    filter_type: "RequestHeaderModifier".to_string(),
                    ..Filter::default()
                }],
                backend_refs: vec![BackendRef {
                    namespace: "default".to_string().into(),
                    name: "users".to_string().into(),
                    port: 8080,
                    filters: vec![Filter {
                        filter_type: "RequestHeaderModifier".to_string(),
                        ..Filter::default()
                    }],
                    ..BackendRef::default()
                }],
                timeouts: None,
                retry: None,
                session_persistence: None,
            }],
            labels: BTreeMap::new(),
            annotations: BTreeMap::new(),
        }],
        backends: vec![BackendCluster {
            name: "users:8080".to_string().into(),
            namespace: "default".to_string().into(),
            protocol: "HTTP".to_string().into(),
            endpoints: vec![BackendEndpoint {
                address: "10.0.0.70".to_string(),
                port: 8080,
                healthy: true,
            }],
            wasm_plugin: None,
                ai_service: None,
                token_policy: None,
        
                circuit_breaker: None,}],
        ..Snapshot::default()
    };

    let selected = snapshot
        .select_http_backend(&RequestMeta::new(
            Some("api.example.com".to_string()),
            "/",
            "GET",
            BTreeMap::new(),
        ))
        .expect("backend");

    assert_eq!(selected.filters.len(), 2);
    assert_eq!(selected.filters[0].filter_type, "RequestHeaderModifier");
    assert_eq!(selected.filters[1].filter_type, "RequestHeaderModifier");
}
