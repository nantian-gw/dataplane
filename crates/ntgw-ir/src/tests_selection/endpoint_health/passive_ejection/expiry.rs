#[test]
fn expired_endpoint_ejection_reintroduces_endpoint_into_rotation() {
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
        backends: vec![BackendCluster {
            name: "echo:8080".to_string(),
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
        ..Snapshot::default()
    };
    let request = RequestMeta::new(
        Some("api.example.com".to_string()),
        "/",
        "GET",
        BTreeMap::new(),
    );

    let selected = snapshot.select_backend(&request).expect("backend");
    let recovered_failure_at =
        Instant::now() - PASSIVE_EJECTION_COOLDOWN - Duration::from_millis(1);
    for _ in 0..PASSIVE_EJECTION_CONSECUTIVE_FAILURES {
        snapshot.record_endpoint_failure_at(&selected, recovered_failure_at);
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
