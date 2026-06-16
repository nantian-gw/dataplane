#[test]
fn selects_grpc_backend_for_catch_all_rule_without_parseable_grpc_path() {
    let snapshot = Snapshot {
        grpc_routes: vec![GrpcRoute {
            name: "grpc-catch-all".to_string(),
            namespace: "default".to_string(),
            hostnames: vec!["grpc.example.com".to_string()],
            parent_refs: vec![],
            rules: vec![GrpcRule {
                name: String::new(),
                matches: vec![],
                filters: vec![],
                backend_refs: vec![backend_ref("default", "greeter", 9090)],
                session_persistence: None,
            }],
            labels: BTreeMap::new(),
            annotations: BTreeMap::new(),
        }],
        backends: vec![BackendCluster {
            name: "greeter:9090".to_string(),
            namespace: "default".to_string(),
            protocol: "GRPC".to_string(),
            endpoints: vec![BackendEndpoint {
                address: "10.0.0.30".to_string(),
                port: 9090,
                healthy: true,
            }],
            wasm_plugin: None,
                ai_service: None,
                token_policy: None,
        }],
        ..Snapshot::default()
    };

    let request = RequestMeta::new(
        Some("grpc.example.com".to_string()),
        "/",
        "POST",
        headers(&[("content-type", "application/grpc")]),
    );

    let selected = snapshot.select_backend(&request).expect("backend");
    assert_eq!(selected.route_kind, RouteKind::Grpc);
    assert_eq!(selected.route_name, "grpc-catch-all");
    assert_eq!(selected.backend.port, 9090);
}

#[test]
fn selects_grpc_backend_for_header_only_match_without_parseable_grpc_path() {
    let snapshot = Snapshot {
        grpc_routes: vec![GrpcRoute {
            name: "grpc-header-only".to_string(),
            namespace: "default".to_string(),
            hostnames: vec!["grpc.example.com".to_string()],
            parent_refs: vec![],
            rules: vec![GrpcRule {
                name: String::new(),
                matches: vec![GrpcMatch {
                    headers: vec![HeaderMatch {
                        name: "x-tenant".to_string(),
                        value: "blue".to_string(),
                        match_type: "Exact".to_string(),
                        ..HeaderMatch::default()
                    }],
                    ..GrpcMatch::default()
                }],
                filters: vec![],
                backend_refs: vec![backend_ref("default", "greeter", 9090)],
                session_persistence: None,
            }],
            labels: BTreeMap::new(),
            annotations: BTreeMap::new(),
        }],
        backends: vec![BackendCluster {
            name: "greeter:9090".to_string(),
            namespace: "default".to_string(),
            protocol: "GRPC".to_string(),
            endpoints: vec![BackendEndpoint {
                address: "10.0.0.30".to_string(),
                port: 9090,
                healthy: true,
            }],
            wasm_plugin: None,
                ai_service: None,
                token_policy: None,
        }],
        ..Snapshot::default()
    };

    let request = RequestMeta::new(
        Some("grpc.example.com".to_string()),
        "/",
        "POST",
        headers(&[("content-type", "application/grpc"), ("x-tenant", "blue")]),
    );

    let selected = snapshot.select_backend(&request).expect("backend");
    assert_eq!(selected.route_kind, RouteKind::Grpc);
    assert_eq!(selected.route_name, "grpc-header-only");
    assert_eq!(selected.backend.port, 9090);
}
