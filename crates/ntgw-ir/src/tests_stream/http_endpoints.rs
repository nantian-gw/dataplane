#[test]
fn rotates_healthy_endpoints_within_cluster() {
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
                    address: "10.0.0.30".to_string(),
                    port: 8080,
                    healthy: true,
                },
                BackendEndpoint {
                    address: "10.0.0.31".to_string(),
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

    let selected = collect_http_addresses(&snapshot, &request, 4);

    assert_eq!(
        selected,
        vec!["10.0.0.30", "10.0.0.31", "10.0.0.30", "10.0.0.31"]
    );
}
