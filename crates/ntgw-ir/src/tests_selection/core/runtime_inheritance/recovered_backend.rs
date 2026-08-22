#[test]
fn inherited_runtime_state_reintroduces_recovered_backend_into_rotation() {
    let current = Snapshot {
        http_routes: vec![HttpRoute {
            name: "weighted".to_string(),
            namespace: "default".to_string(),
            hostnames: vec!["api.example.com".to_string()],
            parent_refs: vec![],
            rules: vec![HttpRule {
                name: String::new(),
                matches: vec![],
                filters: vec![],
                backend_refs: vec![
                    weighted_backend_ref("default", "users", 8080, 1),
                    weighted_backend_ref("default", "orders", 8081, 1),
                ],
                timeouts: None,
                retry: None,
                session_persistence: None,
            }],
            labels: BTreeMap::new(),
            annotations: BTreeMap::new(),
        }],
        backends: vec![
            BackendCluster {
                name: "users:8080".to_string(),
                namespace: "default".to_string(),
                protocol: "HTTP".to_string(),
                endpoints: vec![BackendEndpoint {
                    address: "10.0.0.10".to_string(),
                    port: 8080,
                    healthy: false,
                }],
                wasm_plugin: None,
                ai_service: None,
                token_policy: None,

                circuit_breaker: None,

                security_policy: None,

                },
            BackendCluster {
                name: "orders:8081".to_string(),
                namespace: "default".to_string(),
                protocol: "HTTP".to_string(),
                endpoints: vec![BackendEndpoint {
                    address: "10.0.0.11".to_string(),
                    port: 8081,
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
        Some("api.example.com".to_string()),
        "/",
        "GET",
        BTreeMap::new(),
    );

    let first = current.select_backend(&request).expect("initial backend");
    assert_eq!(first.backend_name, "default/orders:8081");

    let mut next = Snapshot {
        http_routes: current.http_routes.clone(),
        backends: vec![
            BackendCluster {
                name: "users:8080".to_string(),
                namespace: "default".to_string(),
                protocol: "HTTP".to_string(),
                endpoints: vec![BackendEndpoint {
                    address: "10.0.0.10".to_string(),
                    port: 8080,
                    healthy: true,
                }],
                wasm_plugin: None,
                ai_service: None,
                token_policy: None,

                circuit_breaker: None,

                security_policy: None,

                },
            BackendCluster {
                name: "orders:8081".to_string(),
                namespace: "default".to_string(),
                protocol: "HTTP".to_string(),
                endpoints: vec![BackendEndpoint {
                    address: "10.0.0.11".to_string(),
                    port: 8081,
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
    next.inherit_runtime_state_from(&current);

    let selected = collect_http_backends(&next, &request, 4);
    assert_eq!(
        selected
            .iter()
            .filter(|backend| backend.as_str() == "default/users:8080")
            .count(),
        2
    );
    assert_eq!(
        selected
            .iter()
            .filter(|backend| backend.as_str() == "default/orders:8081")
            .count(),
        2
    );
}
