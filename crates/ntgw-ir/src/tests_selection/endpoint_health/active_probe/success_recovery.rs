#[test]
fn active_probe_success_recovers_passively_ejected_endpoint() {
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

                circuit_breaker: None,

                security_policy: None,

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
    for _ in 0..PASSIVE_EJECTION_CONSECUTIVE_FAILURES {
        snapshot.record_endpoint_failure_at(&selected, Instant::now());
    }

    let ejected = collect_http_addresses(&snapshot, &request, 4);
    assert_eq!(
        ejected,
        vec![
            "10.0.0.11".to_string(),
            "10.0.0.11".to_string(),
            "10.0.0.11".to_string(),
            "10.0.0.11".to_string(),
        ]
    );

    snapshot
        .record_endpoint_active_probe_success(selected.backend_name.as_str(), &selected.backend);

    let recovered = collect_http_addresses(&snapshot, &request, 4);
    assert_eq!(
        recovered
            .iter()
            .filter(|address| address.as_str() == "10.0.0.10")
            .count(),
        2
    );
    assert_eq!(
        recovered
            .iter()
            .filter(|address| address.as_str() == "10.0.0.11")
            .count(),
        2
    );
}
