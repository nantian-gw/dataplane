#[test]
fn consistent_hash_by_header_keeps_same_endpoint_for_same_key() {
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
                backend_refs: vec![backend_ref("default", "users", 8080)],
                timeouts: None,
                retry: None,
                session_persistence: None,
            }],
            annotations: BTreeMap::new(),
        }],
        backends: vec![BackendCluster {
            name: "users:8080".to_string(),
            namespace: "default".to_string(),
            protocol: "HTTP".to_string(),
            endpoints: vec![
                BackendEndpoint {
                    address: "10.0.0.10".to_string(),
                    port: 8080,
                    healthy: true,
                },
                BackendEndpoint {
                    address: "10.0.0.11".to_string(),
                    port: 8080,
                    healthy: true,
                },
            ],
            wasm_plugin: None,
                ai_service: None,
                token_policy: None,
        }],
        backend_policies: BTreeMap::from([(
            "default/users:8080".to_string(),
            BackendPolicy {
                load_balancing: Some(LoadBalancingPolicy {
                    policy_type: "ConsistentHash".to_string(),
                    consistent_hash: Some(ConsistentHashPolicy {
                        key_type: "Header".to_string(),
                        header_name: "x-user-id".to_string(),
                    }),
                }),
                ..BackendPolicy::default()
            },
        )]),
        ..Snapshot::default()
    };
    let request = RequestMeta::new(
        Some("api.example.com".to_string()),
        "/",
        "GET",
        headers(&[("x-user-id", "42")]),
    );

    let selected = collect_http_addresses(&snapshot, &request, 8);

    assert_eq!(selected, vec![selected[0].clone(); 8]);
}
