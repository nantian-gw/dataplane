#[test]
fn applies_backend_policy_session_persistence_for_grpc_routes() {
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
                    headers: Vec::new(),
                    ..GrpcMatch::default()
                }],
                filters: vec![],
                backend_refs: vec![
                    weighted_backend_ref("default", "greeter-a", 9090, 1),
                    weighted_backend_ref("default", "greeter-b", 9091, 1),
                ],
                session_persistence: None,
            }],
            annotations: BTreeMap::new(),
        }],
        backends: vec![
            BackendCluster {
                name: "greeter-a:9090".to_string(),
                namespace: "default".to_string(),
                protocol: "GRPC".to_string(),
                endpoints: vec![BackendEndpoint {
                    address: "10.0.0.20".to_string(),
                    port: 9090,
                    healthy: true,
                }],
                wasm_plugin: None,
                ai_service: None,
                token_policy: None,
            },
            BackendCluster {
                name: "greeter-b:9091".to_string(),
                namespace: "default".to_string(),
                protocol: "GRPC".to_string(),
                endpoints: vec![BackendEndpoint {
                    address: "10.0.0.21".to_string(),
                    port: 9091,
                    healthy: true,
                }],
                wasm_plugin: None,
                ai_service: None,
                token_policy: None,
            },
        ],
        backend_policies: BTreeMap::from([(
            "default/greeter-b:9091".to_string(),
            BackendPolicy {
                session_persistence: Some(session_policy("sticky-grpc-backend")),
                ..BackendPolicy::default()
            },
        )]),
        ..Snapshot::default()
    };

    let request = RequestMeta::new(
        Some("grpc.example.com".to_string()),
        "/helloworld.Greeter/SayHello",
        "POST",
        headers(&[("content-type", "application/grpc+proto")]),
    );

    let selected = snapshot
        .select_grpc_backend_with_session_resolver(&request, |policy| {
            (policy.session_name == "sticky-grpc-backend")
                .then(|| session_target("default/greeter-b:9091", "10.0.0.21", 9091))
        })
        .expect("backend");

    assert_eq!(selected.backend_name, "default/greeter-b:9091");
    assert_eq!(
        selected
            .session_persistence
            .as_ref()
            .expect("session persistence")
            .session_name,
        "sticky-grpc-backend"
    );
}
