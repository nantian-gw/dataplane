#[test]
fn inherited_runtime_state_keeps_weighted_selection_progress() {
    let current = Snapshot {
        grpc_routes: vec![GrpcRoute {
            name: "grpc-route".to_string(),
            namespace: "default".to_string(),
            hostnames: vec!["echo.example.com".to_string()],
            parent_refs: vec![],
            rules: vec![GrpcRule {
                name: String::new(),
                matches: vec![GrpcMatch::default()],
                filters: vec![],
                backend_refs: vec![
                    backend_ref("default", "echo-v1", 7070),
                    weighted_backend_ref("default", "echo-v2", 7070, 1),
                ],
                session_persistence: None,
            }],
            labels: BTreeMap::new(),
            annotations: BTreeMap::new(),
        }],
        backends: vec![
            BackendCluster {
                name: "echo-v1:7070".to_string(),
                namespace: "default".to_string(),
                protocol: "GRPC".to_string(),
                endpoints: vec![BackendEndpoint {
                    address: "10.0.0.1".to_string(),
                    port: 7070,
                    healthy: true,
                }],
                wasm_plugin: None,
                ai_service: None,
                token_policy: None,

                circuit_breaker: None,

                security_policy: None,

                },
            BackendCluster {
                name: "echo-v2:7070".to_string(),
                namespace: "default".to_string(),
                protocol: "GRPC".to_string(),
                endpoints: vec![BackendEndpoint {
                    address: "10.0.0.2".to_string(),
                    port: 7070,
                    healthy: true,
                }],
                wasm_plugin: None,
                ai_service: None,
                token_policy: None,

                circuit_breaker: None,

                security_policy: None,

                },
        ],
        ..Snapshot::default()
    };

    let request = RequestMeta::new(
        Some("echo.example.com".to_string()),
        "/gateway_api_conformance.echo_basic.grpcecho.GrpcEcho/Echo",
        "POST",
        headers(&[("content-type", "application/grpc+proto")]),
    );

    let first = current
        .select_backend(&request)
        .expect("first backend selection");
    assert_eq!(first.backend.address, "10.0.0.1");

    let mut next = Snapshot {
        grpc_routes: current.grpc_routes.clone(),
        backends: current.backends.clone(),
        ..Snapshot::default()
    };
    next.inherit_runtime_state_from(&current);

    let second = next
        .select_backend(&request)
        .expect("second backend selection");
    assert_eq!(second.backend.address, "10.0.0.2");
}
