#[test]
fn select_http_backend_preserves_route_timeouts() {
    let snapshot = Snapshot {
        http_routes: vec![HttpRoute {
            name: "timeout-route".to_string().into(),
            namespace: "default".to_string().into(),
            hostnames: vec!["api.example.com".to_string()],
            parent_refs: vec![],
            rules: vec![HttpRule {
                name: String::new(),
                matches: vec![],
                filters: vec![],
                backend_refs: vec![backend_ref("default", "users", 8080)],
                timeouts: Some(crate::RouteTimeouts {
                    request: Some(std::time::Duration::from_secs(12)),
                    backend_request: Some(std::time::Duration::from_secs(3)),
                    connect: None,
                    next_upstream: None,
                }),
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

    assert_eq!(
        selected.timeouts.as_ref().expect("timeouts").request,
        Some(std::time::Duration::from_secs(12))
    );
}

#[test]
fn skips_zero_weight_backend_refs() {
    let snapshot = Snapshot {
        http_routes: vec![HttpRoute {
            name: "weighted".to_string().into(),
            namespace: "default".to_string().into(),
            hostnames: vec!["api.example.com".to_string()],
            parent_refs: vec![],
            rules: vec![HttpRule {
                name: String::new(),
                matches: vec![],
                filters: vec![],
                backend_refs: vec![
                    weighted_backend_ref("default", "disabled", 8080, 0),
                    weighted_backend_ref("default", "active", 8081, 5),
                ],
                timeouts: None,
                retry: None,
                session_persistence: None,
            }],
            labels: BTreeMap::new(),
            annotations: BTreeMap::new(),
        }],
        backends: vec![
            BackendCluster {
                name: "disabled:8080".to_string().into(),
                namespace: "default".to_string().into(),
                protocol: "HTTP".to_string().into(),
                endpoints: vec![BackendEndpoint {
                    address: "10.0.0.20".to_string(),
                    port: 8080,
                    healthy: true,
                }],
                wasm_plugin: None,
                ai_service: None,
                token_policy: None,
            
                circuit_breaker: None,},
            BackendCluster {
                name: "active:8081".to_string().into(),
                namespace: "default".to_string().into(),
                protocol: "HTTP".to_string().into(),
                endpoints: vec![BackendEndpoint {
                    address: "10.0.0.21".to_string(),
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

    let selected = collect_http_backends(&snapshot, &request, 4);

    assert_eq!(selected, vec!["default/active:8081"; 4]);
}

