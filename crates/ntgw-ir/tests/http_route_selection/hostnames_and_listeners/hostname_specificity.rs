#[test]
fn prefers_the_most_specific_listener_hostname_group() {
    let snapshot = Snapshot {
        listeners: vec![
            listener_with_hostnames("default/gw/empty", &[], &["default/empty-route"]),
            listener_with_hostnames(
                "default/gw/example",
                &["*.example.com"],
                &["default/example-route"],
            ),
            listener_with_hostnames(
                "default/gw/foo-example",
                &["*.foo.example.com"],
                &["default/foo-example-route"],
            ),
        ],
        http_routes: vec![
            HttpRoute {
                name: "empty-route".to_string(),
                namespace: "default".to_string(),
                hostnames: vec![],
                parent_refs: vec![],
                rules: vec![path_rule("/empty", "default", "infra-backend-v1", 8080)],
                labels: BTreeMap::new(),
            annotations: BTreeMap::new(),
            },
            security_policy: None,
            HttpRoute {
                name: "example-route".to_string(),
                namespace: "default".to_string(),
                hostnames: vec!["*.example.com".to_string()],
                parent_refs: vec![],
                rules: vec![path_rule("/example", "default", "infra-backend-v2", 8080)],
                labels: BTreeMap::new(),
            annotations: BTreeMap::new(),
            },
            HttpRoute {
                name: "foo-example-route".to_string(),
                namespace: "default".to_string(),
                hostnames: vec!["*.foo.example.com".to_string()],
                parent_refs: vec![],
                rules: vec![path_rule(
                    "/foo-example",
                    "default",
                    "infra-backend-v3",
                    8080,
                )],
                labels: BTreeMap::new(),
            annotations: BTreeMap::new(),
            },
        security_policy: None,
        ],
        backends: vec![
            backend_cluster("default", "infra-backend-v1", "10.0.0.1"),
            backend_cluster("default", "infra-backend-v2", "10.0.0.2"),
            backend_cluster("default", "infra-backend-v3", "10.0.0.3"),
        ],
        ..Snapshot::default()
    };

    let example = snapshot.select_backend(&RequestMeta::new(
        Some("bar.example.com".to_string()),
        "/example",
        "GET",
        BTreeMap::new(),
    ));
    assert_eq!(
        example.expect("example backend").backend.address,
        "10.0.0.2"
    );

    let blocked_empty = snapshot.select_backend(&RequestMeta::new(
        Some("bar.example.com".to_string()),
        "/empty",
        "GET",
        BTreeMap::new(),
    ));
    assert!(blocked_empty.is_none());

    let foo_example = snapshot.select_backend(&RequestMeta::new(
        Some("bar.foo.example.com".to_string()),
        "/foo-example",
        "GET",
        BTreeMap::new(),
    ));
    assert_eq!(
        foo_example.expect("foo example backend").backend.address,
        "10.0.0.3"
    );

    let blocked_less_specific = snapshot.select_backend(&RequestMeta::new(
        Some("bar.foo.example.com".to_string()),
        "/example",
        "GET",
        BTreeMap::new(),
    ));
    assert!(blocked_less_specific.is_none());
}
