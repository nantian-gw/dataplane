#[test]
fn prefers_http_persistent_session_backend_over_weighted_selection() {
    let snapshot = Snapshot {
        http_routes: vec![HttpRoute {
            name: "route".to_string(),
            namespace: "default".to_string(),
            hostnames: vec!["api.example.com".to_string()],
            parent_refs: vec![],
            rules: vec![HttpRule {
                name: String::new(),
                matches: vec![],
                filters: vec![],
                backend_refs: vec![
                    weighted_backend_ref("default", "users", 8080, 1),
                    weighted_backend_ref("default", "orders", 8081, 3),
                ],
                timeouts: None,
                retry: None,
                session_persistence: Some(session_policy("sticky-http")),
            }],
            labels: BTreeMap::new(),
            annotations: BTreeMap::new(),
        }],
        backends: vec![
            BackendCluster {
                name: "users:8080".into(),
                namespace: "default".into(),
                protocol: "HTTP".into(),
                endpoints: vec![BackendEndpoint {
                    address: "10.0.0.10".to_string(),
                    port: 8080,
                    healthy: true,
                }],
                wasm_plugin: None,
                ai_service: None,
                token_policy: None,
            
                circuit_breaker: None,},
            BackendCluster {
                name: "orders:8081".into(),
                namespace: "default".into(),
                protocol: "HTTP".into(),
                endpoints: vec![BackendEndpoint {
                    address: "10.0.0.11".to_string(),
                    port: 8081,
                    healthy: true,
                }],
                wasm_plugin: None,
                ai_service: None,
                token_policy: None,
            
                circuit_breaker: None,},
        ],
        ..Snapshot::default()
    };

    let request = RequestMeta::new(
        Some("api.example.com".to_string()),
        "/",
        "GET",
        BTreeMap::new(),
    );

    let selected = snapshot
        .select_http_backend_with_session_resolver(&request, |_| {
            Some(session_target("default/orders:8081", "10.0.0.11", 8081))
        })
        .expect("backend");

    assert_eq!(selected.backend_name, "default/orders:8081");
    assert_eq!(selected.backend.address, "10.0.0.11");
}

#[test]
fn falls_back_to_weighted_selection_when_session_target_is_unavailable() {
    let snapshot = Snapshot {
        http_routes: vec![HttpRoute {
            name: "route".to_string(),
            namespace: "default".to_string(),
            hostnames: vec!["api.example.com".to_string()],
            parent_refs: vec![],
            rules: vec![HttpRule {
                name: String::new(),
                matches: vec![],
                filters: vec![],
                backend_refs: vec![
                    weighted_backend_ref("default", "users", 8080, 1),
                    weighted_backend_ref("default", "orders", 8081, 3),
                ],
                timeouts: None,
                retry: None,
                session_persistence: Some(session_policy("sticky-http")),
            }],
            labels: BTreeMap::new(),
            annotations: BTreeMap::new(),
        }],
        backends: vec![
            BackendCluster {
                name: "users:8080".into(),
                namespace: "default".into(),
                protocol: "HTTP".into(),
                endpoints: vec![BackendEndpoint {
                    address: "10.0.0.10".to_string(),
                    port: 8080,
                    healthy: true,
                }],
                wasm_plugin: None,
                ai_service: None,
                token_policy: None,
            
                circuit_breaker: None,},
            BackendCluster {
                name: "orders:8081".into(),
                namespace: "default".into(),
                protocol: "HTTP".into(),
                endpoints: vec![BackendEndpoint {
                    address: "10.0.0.11".to_string(),
                    port: 8081,
                    healthy: true,
                }],
                wasm_plugin: None,
                ai_service: None,
                token_policy: None,
            
                circuit_breaker: None,},
        ],
        ..Snapshot::default()
    };

    let request = RequestMeta::new(
        Some("api.example.com".to_string()),
        "/",
        "GET",
        BTreeMap::new(),
    );

    let selected = snapshot
        .select_http_backend_with_session_resolver(&request, |_| {
            Some(session_target("default/orders:8081", "10.0.0.99", 8081))
        })
        .expect("backend");

    assert_eq!(selected.backend_name, "default/users:8080");
    assert_eq!(selected.backend.address, "10.0.0.10");
}
