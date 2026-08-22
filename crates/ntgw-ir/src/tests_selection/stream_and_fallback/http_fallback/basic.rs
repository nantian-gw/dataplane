#[test]
fn falls_back_to_first_healthy_backend() {
    let snapshot = Snapshot {
        backends: vec![BackendCluster {
            name: "echo:8080".to_string(),
            namespace: "default".to_string(),
            protocol: "HTTP".to_string(),
            endpoints: vec![BackendEndpoint {
                address: "127.0.0.1".to_string(),
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

    let selected = snapshot
        .select_backend(&RequestMeta::new(None, "/", "GET", BTreeMap::new()))
        .expect("backend");
    assert_eq!(selected.backend.port, 8080);
}
