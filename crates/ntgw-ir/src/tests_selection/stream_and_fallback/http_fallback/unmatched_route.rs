#[test]
fn returns_none_when_routes_exist_but_request_does_not_match() {
    let snapshot = Snapshot {
        http_routes: vec![HttpRoute {
            name: "route".to_string().into(),
            namespace: "default".to_string().into(),
            hostnames: vec!["api.example.com".to_string()],
            parent_refs: vec![],
            rules: vec![HttpRule {
                name: String::new(),
                matches: vec![HttpMatch {
                    path: "/users".to_string(),
                    path_type: "Exact".to_string(),
                    method: "GET".to_string(),
                    headers: vec![],
                    query_params: vec![],
                    ..HttpMatch::default()
                }],
                filters: vec![],
                backend_refs: vec![backend_ref("default", "users", 8080)],
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
                address: "10.0.0.10".to_string(),
                port: 8080,
                healthy: true,
            }],
            wasm_plugin: None,
                ai_service: None,
                token_policy: None,
        
                circuit_breaker: None,}],
        ..Snapshot::default()
    };

    let selected = snapshot.select_backend(&RequestMeta::new(
        Some("api.example.com".to_string()),
        "/orders",
        "GET",
        BTreeMap::new(),
    ));

    assert!(selected.is_none());
}
