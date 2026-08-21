#[test]
fn selects_grpc_backend_by_service_method_and_header() {
    let snapshot = Snapshot {
        grpc_routes: vec![GrpcRoute {
            name: "grpc-route".to_string(),
            namespace: "default".to_string(),
            hostnames: vec!["grpc.example.com".to_string()],
            parent_refs: vec![],
            rules: vec![GrpcRule {
                name: String::new(),
                matches: vec![GrpcMatch {
                    service: "helloworld.Greeter".to_string(),
                    method: "SayHello".to_string(),
                    match_type: "Exact".to_string(),
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
            name: "greeter:9090".to_string().into(),
            namespace: "default".to_string().into(),
            protocol: "GRPC".to_string().into(),
            endpoints: vec![BackendEndpoint {
                address: "10.0.0.30".to_string(),
                port: 9090,
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
        Some("grpc.example.com".to_string()),
        "/helloworld.Greeter/SayHello",
        "POST",
        headers(&[
            ("content-type", "application/grpc+proto"),
            ("x-tenant", "blue"),
        ]),
    );

    let selected = snapshot.select_backend(&request).expect("backend");
    assert_eq!(selected.route_kind, RouteKind::Grpc);
    assert_eq!(selected.route_name, "grpc-route");
    assert_eq!(selected.backend.port, 9090);
}
