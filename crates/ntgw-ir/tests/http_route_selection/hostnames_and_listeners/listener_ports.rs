#[test]
fn selects_listener_attachments_by_request_port() {
    let snapshot = Snapshot {
        listeners: vec![
            Listener {
                name: "default/gw/http-80".to_string(),
                address: "0.0.0.0".to_string(),
                addresses: vec!["0.0.0.0".to_string()],
                port: 80,
                protocol: "LISTENER_PROTOCOL_HTTP".to_string(),
                hostnames: vec!["foo.com".to_string()],
                attached_routes: vec!["default/backend-v1".to_string()],
                tls: None,
                backend_tls: None,
                metadata: BTreeMap::new(),
            },
            security_policy: None,
            Listener {
                name: "default/gw/http-8080".to_string(),
                address: "0.0.0.0".to_string(),
                addresses: vec!["0.0.0.0".to_string()],
                port: 8080,
                protocol: "LISTENER_PROTOCOL_HTTP".to_string(),
                hostnames: vec!["foo.com".to_string()],
                attached_routes: vec!["default/backend-v2".to_string()],
                tls: None,
                backend_tls: None,
                metadata: BTreeMap::new(),
            },
        ],
        http_routes: vec![
            HttpRoute {
                name: "backend-v1".to_string(),
                namespace: "default".to_string(),
                hostnames: vec!["foo.com".to_string()],
                parent_refs: vec![],
                rules: vec![path_rule("/", "default", "infra-backend-v1", 8080)],
                labels: BTreeMap::new(),
            annotations: BTreeMap::new(),
            },
            security_policy: None,
            HttpRoute {
                name: "backend-v2".to_string(),
                namespace: "default".to_string(),
                hostnames: vec!["foo.com".to_string()],
                parent_refs: vec![],
                rules: vec![path_rule("/", "default", "infra-backend-v2", 8080)],
                labels: BTreeMap::new(),
            annotations: BTreeMap::new(),
            },
        ],
        backends: vec![
            backend_cluster("default", "infra-backend-v1", "10.0.0.1"),
            backend_cluster("default", "infra-backend-v2", "10.0.0.2"),
        ],
        ..Snapshot::default()
    };

    let http = snapshot
        .select_backend(&RequestMeta::with_port(
            Some("foo.com".to_string()),
            80,
            "/",
            "GET",
            BTreeMap::new(),
        ))
        .expect("listener 80 backend");
    assert_eq!(http.backend.address, "10.0.0.1");

    let alt = snapshot
        .select_backend(&RequestMeta::new(
            Some("foo.com:8080".to_string()),
            "/",
            "GET",
            BTreeMap::new(),
        ))
        .expect("listener 8080 backend");
    assert_eq!(alt.backend.address, "10.0.0.2");
}
