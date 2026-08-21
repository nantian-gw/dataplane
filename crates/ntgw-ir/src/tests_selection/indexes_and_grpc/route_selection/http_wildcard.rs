#[test]
fn selects_http_backend_with_wildcard_hostname() {
    let snapshot = Snapshot {
        http_routes: vec![HttpRoute {
            name: "wildcard".to_string(),
            namespace: "default".to_string(),
            hostnames: vec!["*.example.com".to_string()],
            parent_refs: vec![],
            rules: vec![HttpRule {
                name: String::new(),
                matches: vec![],
                filters: vec![],
                backend_refs: vec![backend_ref("default", "wild", 8080)],
                timeouts: None,
                retry: None,
                session_persistence: None,
            }],
            labels: BTreeMap::new(),
            annotations: BTreeMap::new(),
        }],
        backends: vec![BackendCluster {
            name: "wild:8080".to_string().into(),
            namespace: "default".to_string().into(),
            protocol: "HTTP".to_string().into(),
            endpoints: vec![BackendEndpoint {
                address: "10.0.0.20".to_string(),
                port: 8080,
                healthy: true,
            }],
            wasm_plugin: None,
                ai_service: None,
                token_policy: None,

                circuit_breaker: None,

                security_policy: None,

                }],
        ..Snapshot::default()
    };

    let request = RequestMeta::new(
        Some("foo.example.com:443".to_string()),
        "/",
        "GET",
        BTreeMap::new(),
    );

    let selected = snapshot.select_backend(&request).expect("backend");
    assert_eq!(selected.route_name, "wildcard");
}
