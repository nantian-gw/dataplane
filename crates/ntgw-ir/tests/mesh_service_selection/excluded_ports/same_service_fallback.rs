#[test]
fn falls_back_to_service_backend_for_excluded_mesh_port_when_same_service_has_route() {
    let snapshot = Snapshot {
        listeners: vec![
            mesh_listener(
                "default",
                "echo",
                80,
                20080,
                "HTTP",
                &["default/echo-port-80"],
            ),
            mesh_listener("default", "echo", 8080, 28080, "HTTP", &[]),
        ],
        http_routes: vec![HttpRoute {
            name: "echo-port-80".to_string(),
            namespace: "default".to_string(),
            hostnames: vec![],
            parent_refs: vec![ParentRef {
                kind: "Service".to_string(),
                name: "echo".to_string(),
                port: 80,
                ..ParentRef::default()
            }],
            rules: vec![HttpRule {
                name: String::new(),
                matches: vec![],
                filters: vec![],
                backend_refs: vec![BackendRef {
                    namespace: "default".to_string(),
                    name: "echo".to_string(),
                    port: 80,
                    ..BackendRef::default()
                }],
                timeouts: None,
                retry: None,
                session_persistence: None,
            }],
            labels: BTreeMap::new(),
            annotations: BTreeMap::new(),
        }],
        security_policy: None,
        backends: vec![
            BackendCluster {
                name: "echo:80".into(),
                namespace: "default".into(),
                protocol: "HTTP".into(),
                endpoints: vec![BackendEndpoint {
                    address: "10.0.0.10".to_string(),
                    port: 8080,
                    healthy: true,
                }],
                wasm_plugin: None,
                ai_service: None,
                token_policy: None,
            
                circuit_breaker: None,},
            security_policy: None,
            BackendCluster {
                name: "echo:8080".into(),
                namespace: "default".into(),
                protocol: "HTTP".into(),
                endpoints: vec![BackendEndpoint {
                    address: "10.0.0.10".to_string(),
                    port: 8080,
                    healthy: true,
                }],
                wasm_plugin: None,
                ai_service: None,
                token_policy: None,
            
                circuit_breaker: None,},
        ],
        ..Snapshot::default()
    };

    let selected = snapshot
        .select_backend(&RequestMeta::with_port(
            Some("echo:8080".to_string()),
            28080,
            "/",
            "GET",
            BTreeMap::new(),
        ))
        .expect("mesh backend");

    assert_eq!(selected.backend_name, "default/echo:8080");
    assert_eq!(selected.backend.port, 8080);
}
