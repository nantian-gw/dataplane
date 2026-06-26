#[test]
fn does_not_fall_back_for_mesh_grpc_service_request_without_routes() {
    let snapshot = Snapshot {
        backends: vec![
            BackendCluster {
                name: "echo-v1:7070".to_string(),
                namespace: "default".to_string(),
                protocol: "GRPC".to_string(),
                endpoints: vec![BackendEndpoint {
                    address: "10.0.0.11".to_string(),
                    port: 7070,
                    healthy: true,
                }],
                wasm_plugin: None,
                ai_service: None,
                token_policy: None,
            
                circuit_breaker: None,},
            BackendCluster {
                name: "echo-v2:7070".to_string(),
                namespace: "default".to_string(),
                protocol: "GRPC".to_string(),
                endpoints: vec![BackendEndpoint {
                    address: "10.0.0.12".to_string(),
                    port: 7070,
                    healthy: true,
                }],
                wasm_plugin: None,
                ai_service: None,
                token_policy: None,
            
                circuit_breaker: None,},
            BackendCluster {
                name: "echo:7070".to_string(),
                namespace: "default".to_string(),
                protocol: "GRPC".to_string(),
                endpoints: vec![BackendEndpoint {
                    address: "10.0.0.13".to_string(),
                    port: 7070,
                    healthy: true,
                }],
                wasm_plugin: None,
                ai_service: None,
                token_policy: None,
            
                circuit_breaker: None,},
        ],
        ..Snapshot::default()
    };

    let request = RequestMeta {
        host: Some("echo".to_string()),
        port: 7070,
        path: "/gateway_api_conformance.echo_basic.grpcecho.GrpcEcho/Echo".to_string(),
        method: "POST".to_string(),
        source_ip: None,
        headers: headers(&[("content-type", "application/grpc+proto")]),
        query_params: BTreeMap::new(),
    };

    assert!(
        snapshot.select_backend(&request).is_none(),
        "mesh service requests without an attached GRPCRoute must not fall back to an arbitrary backend"
    );
}
