#[test]
fn endpoint_success_clears_passive_failure_streak() {
    let snapshot = Snapshot {
        http_routes: vec![HttpRoute {
            name: "route".to_string(),
            namespace: "default".to_string(),
            hostnames: vec!["api.example.com".to_string()],
            parent_refs: vec![],
            rules: vec![HttpRule {
                name: String::new(),
                matches: vec![],
                filters: vec![],
                backend_refs: vec![backend_ref("default", "echo", 8080)],
                timeouts: None,
                retry: None,
                session_persistence: None,
            }],
            labels: BTreeMap::new(),
            annotations: BTreeMap::new(),
        }],
        security_policy: None,
        backends: vec![BackendCluster {
            name: "echo:8080".to_string().into(),
            namespace: "default".to_string().into(),
            protocol: "HTTP".to_string().into(),
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
        
                circuit_breaker: None,}],
        ..Snapshot::default()
    };
    let request = RequestMeta::new(
        Some("api.example.com".to_string()),
        "/",
        "GET",
        BTreeMap::new(),
    );

    let selected = snapshot.select_backend(&request).expect("backend");
    for _ in 0..(PASSIVE_EJECTION_CONSECUTIVE_FAILURES - 1) {
        snapshot.record_endpoint_failure_at(&selected, Instant::now());
    }
    snapshot.record_endpoint_success(&selected);
    for _ in 0..(PASSIVE_EJECTION_CONSECUTIVE_FAILURES - 1) {
        snapshot.record_endpoint_failure_at(&selected, Instant::now());
    }

    let addresses = collect_http_addresses(&snapshot, &request, 4);
    assert_eq!(
        addresses
            .iter()
            .filter(|address| address.as_str() == "10.0.0.10")
            .count(),
        2
    );
    assert_eq!(
        addresses
            .iter()
            .filter(|address| address.as_str() == "10.0.0.11")
            .count(),
        2
    );
}
