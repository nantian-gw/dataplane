use super::*;

#[test]
fn selects_most_specific_header_match_on_attached_listener() {
    let snapshot = Snapshot {
        listeners: vec![Listener {
            name: "default/gw/http".to_string(),
            address: "0.0.0.0".to_string(),
            addresses: vec!["0.0.0.0".to_string()],
            port: 80,
            protocol: "LISTENER_PROTOCOL_HTTP".to_string(),
            hostnames: vec![],
            attached_routes: vec!["default/header-matching".to_string()],
            tls: None,
            backend_tls: None,
            metadata: BTreeMap::new(),
        }],
        http_routes: vec![HttpRoute {
            name: "header-matching".to_string(),
            namespace: "default".to_string(),
            hostnames: vec![],
            parent_refs: vec![],
            rules: vec![
                HttpRule {
                    name: String::new(),
                    matches: vec![HttpMatch {
                        headers: vec![HeaderMatch {
                            name: "version".to_string(),
                            value: "two".to_string(),
                            match_type: "Exact".to_string(),
                            ..HeaderMatch::default()
                        }],
                        ..HttpMatch::default()
                    }],
                    filters: vec![],
                    backend_refs: vec![backend_ref("default", "infra-backend-v2", 8080)],
                    timeouts: None,
                    retry: None,
                    session_persistence: None,
                },
                HttpRule {
                    name: String::new(),
                    matches: vec![HttpMatch {
                        headers: vec![
                            HeaderMatch {
                                name: "version".to_string(),
                                value: "two".to_string(),
                                match_type: "Exact".to_string(),
                                ..HeaderMatch::default()
                            },
                            HeaderMatch {
                                name: "color".to_string(),
                                value: "orange".to_string(),
                                match_type: "Exact".to_string(),
                                ..HeaderMatch::default()
                            },
                        ],
                        ..HttpMatch::default()
                    }],
                    filters: vec![],
                    backend_refs: vec![backend_ref("default", "infra-backend-v1", 8080)],
                    timeouts: None,
                    retry: None,
                    session_persistence: None,
                },
            ],
            labels: BTreeMap::new(),
            annotations: BTreeMap::new(),
        }],
        backends: vec![
            backend_cluster("default", "infra-backend-v1", "10.0.0.1"),
            backend_cluster("default", "infra-backend-v2", "10.0.0.2"),
        ],
        ..Snapshot::default()
    };

    let selected = snapshot
        .select_backend(&RequestMeta::new(
            Some("example.com".to_string()),
            "/",
            "GET",
            headers(&[("version", "two"), ("color", "orange")]),
        ))
        .expect("backend");

    assert_eq!(selected.route_name, "header-matching");
    assert_eq!(selected.backend.address, "10.0.0.1");
}

#[test]
fn prefers_lexicographically_earlier_route_when_http_scores_tie() {
    let snapshot = Snapshot {
        listeners: vec![listener_with_hostnames(
            "default/gw/http",
            &["api.example.com"],
            &["default/a-route", "default/z-route"],
        )],
        http_routes: vec![
            HttpRoute {
                name: "z-route".to_string(),
                namespace: "default".to_string(),
                hostnames: vec!["api.example.com".to_string()],
                parent_refs: vec![],
                rules: vec![path_rule("/", "default", "backend-z", 8080)],
                labels: BTreeMap::new(),
            annotations: BTreeMap::new(),
            },
            HttpRoute {
                name: "a-route".to_string(),
                namespace: "default".to_string(),
                hostnames: vec!["api.example.com".to_string()],
                parent_refs: vec![],
                rules: vec![path_rule("/", "default", "backend-a", 8080)],
                labels: BTreeMap::new(),
            annotations: BTreeMap::new(),
            },
        ],
        backends: vec![
            backend_cluster("default", "backend-a", "10.0.0.12"),
            backend_cluster("default", "backend-z", "10.0.0.13"),
        ],
        ..Snapshot::default()
    };

    let mut snapshot = snapshot;
    snapshot.http_routes.sort_by(|left, right| {
        (left.namespace.as_str(), left.name.as_str())
            .cmp(&(right.namespace.as_str(), right.name.as_str()))
    });

    let selected = snapshot
        .select_backend(&RequestMeta::new(
            Some("api.example.com".to_string()),
            "/",
            "GET",
            BTreeMap::new(),
        ))
        .expect("selected backend");

    assert_eq!(selected.route_name, "a-route");
    assert_eq!(selected.backend.address, "10.0.0.12");
}
