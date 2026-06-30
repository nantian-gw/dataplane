#[test]
fn applies_backend_policy_session_persistence_without_existing_token() {
    let snapshot = Snapshot {
        http_routes: vec![HttpRoute {
            name: "route".to_string().into(),
            namespace: "default".to_string().into(),
            hostnames: vec!["api.example.com".to_string()],
            parent_refs: vec![],
            rules: vec![HttpRule {
                name: String::new(),
                matches: vec![],
                filters: vec![],
                backend_refs: vec![weighted_backend_ref("default", "orders", 8081, 1)],
                timeouts: None,
                retry: None,
                session_persistence: None,
            }],
            labels: BTreeMap::new(),
            annotations: BTreeMap::new(),
        }],
        backends: vec![BackendCluster {
            name: "orders:8081".into(),
            namespace: "default".into(),
            protocol: "HTTP".into(),
            endpoints: vec![BackendEndpoint {
                address: "10.0.0.11".to_string(),
                port: 8081,
                healthy: true,
            }],
            wasm_plugin: None,
                ai_service: None,
                token_policy: None,
        
                circuit_breaker: None,}],
        backend_policies: BTreeMap::from([(
            "default/orders:8081".to_string(),
            BackendPolicy {
                session_persistence: Some(session_policy("sticky-orders")),
                ..BackendPolicy::default()
            },
        )]),
        ..Snapshot::default()
    };

    let request = RequestMeta::new(
        Some("api.example.com".to_string()),
        "/",
        "GET",
        BTreeMap::new(),
    );

    let selected = snapshot
        .select_http_backend_with_session_resolver(&request, |_| None)
        .expect("backend");

    assert_eq!(selected.backend_name, "default/orders:8081");
    assert_eq!(
        selected
            .session_persistence
            .as_ref()
            .expect("session persistence")
            .session_name,
        "sticky-orders"
    );
}
