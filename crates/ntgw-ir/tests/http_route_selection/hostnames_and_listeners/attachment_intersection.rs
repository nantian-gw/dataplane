#[test]
fn ignores_routes_not_attached_to_matching_listener_hostname() {
    let snapshot = Snapshot {
        listeners: vec![
            Listener {
                name: "default/gw/specific".to_string(),
                address: "0.0.0.0".to_string(),
                addresses: vec!["0.0.0.0".to_string()],
                port: 80,
                protocol: "LISTENER_PROTOCOL_HTTP".to_string(),
                hostnames: vec!["very.specific.com".to_string()],
                attached_routes: vec!["default/specific-route".to_string()],
                tls: None,
                backend_tls: None,
                metadata: BTreeMap::new(),
            },
            Listener {
                name: "default/gw/wildcard".to_string(),
                address: "0.0.0.0".to_string(),
                addresses: vec!["0.0.0.0".to_string()],
                port: 80,
                protocol: "LISTENER_PROTOCOL_HTTP".to_string(),
                hostnames: vec!["*.wildcard.io".to_string()],
                attached_routes: vec!["default/wildcard-route".to_string()],
                tls: None,
                backend_tls: None,
                metadata: BTreeMap::new(),
            },
        ],
        http_routes: vec![
            HttpRoute {
                name: "specific-route".to_string(),
                namespace: "default".to_string(),
                hostnames: vec!["very.specific.com".to_string()],
                parent_refs: vec![],
                rules: vec![path_rule("/s1", "default", "infra-backend-v1", 8080)],
                annotations: BTreeMap::new(),
            },
            HttpRoute {
                name: "wildcard-route".to_string(),
                namespace: "default".to_string(),
                hostnames: vec!["foo.wildcard.io".to_string()],
                parent_refs: vec![],
                rules: vec![path_rule("/s2", "default", "infra-backend-v2", 8080)],
                annotations: BTreeMap::new(),
            },
            HttpRoute {
                name: "non-intersecting-route".to_string(),
                namespace: "default".to_string(),
                hostnames: vec!["wildcard.io".to_string()],
                parent_refs: vec![],
                rules: vec![path_rule("/s2", "default", "infra-backend-v3", 8080)],
                annotations: BTreeMap::new(),
            },
        ],
        backends: vec![
            backend_cluster("default", "infra-backend-v1", "10.0.0.1"),
            backend_cluster("default", "infra-backend-v2", "10.0.0.2"),
            backend_cluster("default", "infra-backend-v3", "10.0.0.3"),
        ],
        ..Snapshot::default()
    };

    let wildcard = snapshot
        .select_backend(&RequestMeta::new(
            Some("foo.wildcard.io".to_string()),
            "/s2",
            "GET",
            BTreeMap::new(),
        ))
        .expect("wildcard route");
    assert_eq!(wildcard.backend.address, "10.0.0.2");

    let blocked = snapshot.select_backend(&RequestMeta::new(
        Some("wildcard.io".to_string()),
        "/s2",
        "GET",
        BTreeMap::new(),
    ));
    assert!(blocked.is_none());
}
