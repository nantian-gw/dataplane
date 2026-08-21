#[test]
fn does_not_select_grpc_backend_for_non_grpc_request_with_parseable_path() {
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
        security_policy: None,
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
        
                circuit_breaker: None,}],
        ..Snapshot::default()
    };

    let request = RequestMeta::new(
        Some("grpc.example.com".to_string()),
        "/helloworld.Greeter/SayHello",
        "POST",
        BTreeMap::new(),
    );

    assert!(
        snapshot.select_grpc_backend(&request).is_none(),
        "plain HTTP requests must not match GRPCRoute rules"
    );
}
