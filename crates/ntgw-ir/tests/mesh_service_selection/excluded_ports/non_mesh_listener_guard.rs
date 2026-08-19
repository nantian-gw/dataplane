#[test]
fn excluded_mesh_port_fallback_ignores_non_mesh_listeners_when_checking_attached_routes() {
    let snapshot = Snapshot {
        listeners: vec![
            Listener {
                name: "default/gw/http".to_string(),
                address: "0.0.0.0".to_string(),
                port: 80,
                protocol: "HTTP".to_string(),
                attached_routes: vec!["default/gateway-route".to_string()],
                ..Listener::default()
            },
            security_policy: None,
            mesh_listener(
                "gateway-conformance-mesh",
                "echo-v1",
                80,
                20080,
                "HTTP",
                &["gateway-conformance-mesh/mesh-split-v1"],
            ),
            mesh_listener(
                "gateway-conformance-mesh",
                "echo-v1",
                8080,
                28080,
                "HTTP",
                &[],
            ),
        ],
        http_routes: vec![HttpRoute {
            name: "mesh-split-v1".to_string(),
            namespace: "gateway-conformance-mesh".to_string(),
            hostnames: vec![],
            parent_refs: vec![ParentRef {
                kind: "Service".to_string(),
                name: "echo-v1".to_string(),
                port: 80,
                ..ParentRef::default()
            }],
            rules: vec![HttpRule {
                name: String::new(),
                matches: vec![],
                filters: vec![],
                backend_refs: vec![BackendRef {
                    namespace: "gateway-conformance-mesh".to_string(),
                    name: "echo-v1".to_string(),
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
                name: "echo-v1:80".into(),
                namespace: "gateway-conformance-mesh".into(),
                protocol: "HTTP".into(),
                endpoints: vec![BackendEndpoint {
                    address: "10.0.0.11".to_string(),
                    port: 8080,
                    healthy: true,
                }],
                wasm_plugin: None,
                ai_service: None,
                token_policy: None,
            
                circuit_breaker: None,},
            security_policy: None,
            BackendCluster {
                name: "echo-v1:8080".into(),
                namespace: "gateway-conformance-mesh".into(),
                protocol: "HTTP".into(),
                endpoints: vec![BackendEndpoint {
                    address: "10.0.0.11".to_string(),
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
            Some("echo-v1:8080".to_string()),
            28080,
            "/",
            "GET",
            BTreeMap::new(),
        ))
        .expect("mesh backend");

    assert_eq!(
        selected.backend_name,
        "gateway-conformance-mesh/echo-v1:8080"
    );
    assert_eq!(selected.backend.port, 8080);
}
