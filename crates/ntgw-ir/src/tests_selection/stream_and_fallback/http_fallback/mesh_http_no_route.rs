#[test]
fn does_not_fall_back_for_mesh_http_service_request_without_routes() {
    let snapshot = Snapshot {
        backends: vec![
            BackendCluster {
                name: "echo-v1:8080".to_string().into(),
                namespace: "default".to_string().into(),
                protocol: "HTTP".to_string().into(),
                endpoints: vec![BackendEndpoint {
                    address: "10.0.0.21".to_string(),
                    port: 8080,
                    healthy: true,
                }],
                wasm_plugin: None,
                ai_service: None,
                token_policy: None,
            
                circuit_breaker: None,},
            BackendCluster {
                name: "echo:8080".to_string().into(),
                namespace: "default".to_string().into(),
                protocol: "HTTP".to_string().into(),
                endpoints: vec![BackendEndpoint {
                    address: "10.0.0.22".to_string(),
                    port: 8080,
                    healthy: true,
                }],
                wasm_plugin: None,
                ai_service: None,
                token_policy: None,
            
                circuit_breaker: None,},
        ],
        workloads: vec![Workload {
            namespace: "default".to_string().into(),
            name: "client".to_string().into(),
            ip: "10.1.2.4".to_string(),
        }],
        ..Snapshot::default()
    };

    let request = RequestMeta {
        host: Some("echo.default.svc.cluster.local".to_string()),
        port: 8080,
        path: "/".to_string(),
        method: "GET".to_string(),
        source_ip: Some("10.1.2.4".to_string()),
        headers: BTreeMap::new(),
        query_params: BTreeMap::new(),
    };

    assert!(
        snapshot.select_backend(&request).is_none(),
        "mesh service requests without an attached HTTPRoute must not fall back to an arbitrary backend"
    );
}
