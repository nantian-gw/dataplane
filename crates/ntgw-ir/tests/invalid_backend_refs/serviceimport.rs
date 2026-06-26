use super::*;

#[test]
fn treats_serviceimport_backend_refs_as_routable() {
    let snapshot = Snapshot {
        listeners: vec![listener("default/gw/http", "default/imported")],
        http_routes: vec![HttpRoute {
            name: "imported".to_string(),
            namespace: "default".to_string(),
            hostnames: vec![],
            parent_refs: vec![],
            rules: vec![HttpRule {
                name: String::new(),
                backend_refs: vec![BackendRef {
                    group: "multicluster.x-k8s.io".to_string(),
                    kind: "ServiceImport".to_string(),
                    namespace: "default".to_string(),
                    name: "payments".to_string(),
                    port: 9443,
                    ..BackendRef::default()
                }],
                ..HttpRule::default()
            }],
            labels: BTreeMap::new(),
            annotations: BTreeMap::new(),
        }],
        backends: vec![BackendCluster {
            name: "payments:9443".to_string(),
            namespace: "default".to_string(),
            protocol: "HTTP".to_string(),
            endpoints: vec![BackendEndpoint {
                address: "10.0.0.44".to_string(),
                port: 19443,
                healthy: true,
            }],
            wasm_plugin: None,
            ai_service: None,
            token_policy: None,

            circuit_breaker: None,
        }],
        ..Snapshot::default()
    };

    let selected = snapshot
        .select_http_route(&RequestMeta::new(None, "/", "GET", BTreeMap::new()))
        .expect("matched route");

    assert_eq!(
        selected.backend_name.as_deref(),
        Some("default/payments:9443")
    );
    assert_eq!(
        selected.backend.as_ref().map(|backend| backend.port),
        Some(19443)
    );
}
