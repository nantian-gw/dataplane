#[test]
fn selects_http_backend_by_host_path_method_header_and_query() {
    let snapshot = Snapshot {
        http_routes: vec![HttpRoute {
            name: "route".to_string(),
            namespace: "default".to_string(),
            hostnames: vec!["api.example.com".to_string()],
            parent_refs: vec![],
            rules: vec![HttpRule {
                name: String::new(),
                matches: vec![HttpMatch {
                    path: "/users".to_string(),
                    path_type: "PathPrefix".to_string(),
                    method: "GET".to_string(),
                    headers: vec![HeaderMatch {
                        name: "x-env".to_string(),
                        value: "prod".to_string(),
                        match_type: "Exact".to_string(),
                        ..HeaderMatch::default()
                    }],
                    query_params: vec![QueryMatch {
                        name: "debug".to_string(),
                        value: "false".to_string(),
                        match_type: "Exact".to_string(),
                        ..QueryMatch::default()
                    }],
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

                circuit_breaker: None,

                security_policy: None,

                }],
        ..Snapshot::default()
    };

    let request = RequestMeta::new(
        Some("api.example.com".to_string()),
        "/users?id=123&debug=false",
        "GET",
        headers(&[("x-env", "prod"), ("content-type", "application/json")]),
    );

    let selected = snapshot.select_backend(&request).expect("backend");
    assert_eq!(selected.route_kind, RouteKind::Http);
    assert_eq!(selected.route_name, "route");
    assert_eq!(selected.backend.address, "10.0.0.10");
}
