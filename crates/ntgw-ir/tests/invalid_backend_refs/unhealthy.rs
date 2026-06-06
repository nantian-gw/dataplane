use super::*;

#[test]
fn reports_unhealthy_backends_when_route_matches_without_healthy_endpoints() {
    let snapshot = Snapshot {
        listeners: vec![listener("default/gw/http", "default/unhealthy")],
        http_routes: vec![HttpRoute {
            name: "unhealthy".to_string(),
            namespace: "default".to_string(),
            hostnames: vec![],
            parent_refs: vec![],
            rules: vec![HttpRule {
                name: String::new(),
                backend_refs: vec![BackendRef {
                    namespace: "default".to_string(),
                    name: "infra-backend-v1".to_string(),
                    port: 8080,
                    ..BackendRef::default()
                }],
                ..HttpRule::default()
            }],
            annotations: BTreeMap::new(),
        }],
        backends: vec![backend_cluster("default", "infra-backend-v1", false)],
        ..Snapshot::default()
    };

    let selected = snapshot
        .select_http_route(&RequestMeta::new(None, "/", "GET", BTreeMap::new()))
        .expect("matched route");

    assert!(selected.backend.is_none());
    assert_eq!(
        selected.backend_error,
        Some(BackendSelectionError::NoHealthyBackends)
    );
}
