use super::*;

#[test]
fn marks_invalid_backend_refs_as_route_errors() {
    let snapshot = Snapshot {
        listeners: vec![listener("default/gw/http", "default/invalid-kind")],
        http_routes: vec![HttpRoute {
            name: "invalid-kind".to_string(),
            namespace: "default".to_string(),
            hostnames: vec![],
            parent_refs: vec![],
            rules: vec![HttpRule {
                name: String::new(),
                backend_refs: vec![BackendRef {
                    namespace: "default".to_string(),
                    name: "infra-backend-v1".to_string(),
                    port: 8080,
                    metadata: BTreeMap::from([(
                        "nantian.dev/backend-ref-valid".to_string(),
                        "false".to_string(),
                    )]),
                    ..BackendRef::default()
                }],
                ..HttpRule::default()
            }],
            annotations: BTreeMap::new(),
        }],
        backends: vec![backend_cluster("default", "infra-backend-v1", true)],
        ..Snapshot::default()
    };

    let selected = snapshot
        .select_http_route(&RequestMeta::new(None, "/", "GET", BTreeMap::new()))
        .expect("matched route");

    assert!(selected.backend.is_none());
    assert_eq!(
        selected.backend_error,
        Some(BackendSelectionError::InvalidBackendRefs)
    );
}
