#[test]
fn consistent_hash_by_header_keeps_same_backend_for_same_key() {
    let snapshot = Snapshot {
        http_routes: vec![HttpRoute {
            name: "hash".to_string(),
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
                name: "users:8080".to_string().into(),
                namespace: "default".to_string().into(),
                protocol: "HTTP".to_string().into(),
                endpoints: vec![BackendEndpoint {
                    address: "10.0.0.10".to_string(),
                    port: 8080,
                    healthy: true,
                }],
                wasm_plugin: None,
                ai_service: None,
                token_policy: None,
            
                circuit_breaker: None,},
            BackendCluster {
                name: "orders:8081".to_string().into(),
                namespace: "default".to_string().into(),
                protocol: "HTTP".to_string().into(),
                endpoints: vec![BackendEndpoint {
                    address: "10.0.0.11".to_string(),
                    port: 8081,
                    healthy: true,
                }],
                wasm_plugin: None,
                ai_service: None,
                token_policy: None,
            
                circuit_breaker: None,},
        ],
        backend_policies: BTreeMap::from([
            (
                "default/users:8080".to_string(),
                BackendPolicy {
                    load_balancing: Some(LoadBalancingPolicy {
                        slow_start: None,
                        policy_type: "ConsistentHash".to_string(),
                        consistent_hash: Some(ConsistentHashPolicy {
                            key_type: "Header".to_string(),
                            header_name: "x-user-id".to_string(),
                        }),
                    }),
                    ..BackendPolicy::default()
                },
            ),
            (
                "default/orders:8081".to_string(),
                BackendPolicy {
                    load_balancing: Some(LoadBalancingPolicy {
                        slow_start: None,
                        policy_type: "ConsistentHash".to_string(),
                        consistent_hash: Some(ConsistentHashPolicy {
                            key_type: "Header".to_string(),
                            header_name: "x-user-id".to_string(),
                        }),
                    }),
                    ..BackendPolicy::default()
                },
            ),
        ]),
        ..Snapshot::default()
    };
    let request = RequestMeta::new(
        Some("api.example.com".to_string()),
        "/",
        "GET",
        headers(&[("x-user-id", "42")]),
    );

    let selected = collect_http_backends(&snapshot, &request, 8);

    assert_eq!(selected, vec![selected[0].clone(); 8]);
}
