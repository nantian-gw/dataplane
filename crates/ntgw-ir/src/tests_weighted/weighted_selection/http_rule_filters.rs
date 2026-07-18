#[test]
fn selects_http_backend_refs_by_weighted_round_robin_with_rule_filters() {
    let snapshot = Snapshot {
        http_routes: vec![HttpRoute {
            name: "weighted".to_string(),
            namespace: "default".to_string(),
            hostnames: vec!["api.example.com".to_string()],
            parent_refs: vec![],
            rules: vec![HttpRule {
                name: String::new(),
                matches: vec![],
                filters: vec![Filter {
                    filter_type: "RequestHeaderModifier".to_string(),
                    ..Filter::default()
                }],
                backend_refs: vec![
                    weighted_backend_ref("default", "users", 8080, 1),
                    weighted_backend_ref("default", "orders", 8081, 3),
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
        ..Snapshot::default()
    };
    let request = RequestMeta::new(
        Some("api.example.com".to_string()),
        "/",
        "GET",
        BTreeMap::new(),
    );

    let selections = (0..8)
        .map(|_| snapshot.select_backend(&request).expect("backend"))
        .collect::<Vec<_>>();

    assert_eq!(
        selections
            .iter()
            .map(|selection| selection.backend_name.clone())
            .collect::<Vec<_>>(),
        vec![
            "default/users:8080",
            "default/orders:8081",
            "default/orders:8081",
            "default/orders:8081",
            "default/users:8080",
            "default/orders:8081",
            "default/orders:8081",
            "default/orders:8081",
        ]
    );
    assert!(selections
        .iter()
        .all(|selection| selection.filters.len() == 1
            && selection.filters[0].filter_type == "RequestHeaderModifier"));
}
