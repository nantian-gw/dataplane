use super::*;

#[test]
fn keeps_valid_backends_when_rule_contains_mixed_refs() {
    let snapshot = Snapshot {
        listeners: vec![listener("default/gw/http", "default/mixed")],
        http_routes: vec![HttpRoute {
            name: "mixed".to_string(),
            namespace: "default".to_string(),
            hostnames: vec![],
            parent_refs: vec![],
            rules: vec![HttpRule {
                name: String::new(),
                backend_refs: vec![
                    BackendRef {
                        namespace: "default".to_string(),
                        name: "infra-backend-v1".to_string(),
                        port: 8080,
                        metadata: BTreeMap::from([(
                            "nantian.dev/backend-ref-valid".to_string(),
                            "false".to_string(),
                        )]),
                        ..BackendRef::default()
                    },
                    BackendRef {
                        namespace: "default".to_string(),
                        name: "infra-backend-v2".to_string(),
                        port: 8080,
                        ..BackendRef::default()
                    },
                ],
                ..HttpRule::default()
            }],
            labels: BTreeMap::new(),
            annotations: BTreeMap::new(),
        }],
        security_policy: None,
        backends: vec![
            backend_cluster("default", "infra-backend-v1", true),
            backend_cluster("default", "infra-backend-v2", true),
        ],
        ..Snapshot::default()
    };

    let selected = snapshot
        .select_http_route(&RequestMeta::new(None, "/", "GET", BTreeMap::new()))
        .expect("matched route");

    assert_eq!(
        selected.backend_name.as_deref(),
        Some("default/infra-backend-v2:8080")
    );
    assert_eq!(selected.backend_error, None);
}
