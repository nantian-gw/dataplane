#[test]
fn mesh_service_backend_fallback_still_applies_when_attached_route_is_not_eligible() {
    let snapshot = Snapshot {
        listeners: vec![mesh_listener(
            "gateway-conformance-mesh",
            "echo-v1",
            80,
            20080,
            "HTTP",
            &["gateway-conformance-mesh-consumer/mesh-echo-add-header"],
        )],
        http_routes: vec![HttpRoute {
            name: "mesh-echo-add-header".to_string(),
            namespace: "gateway-conformance-mesh-consumer".to_string(),
            hostnames: vec![],
            parent_refs: vec![ParentRef {
                kind: "Service".to_string(),
                namespace: "gateway-conformance-mesh".to_string(),
                name: "echo-v1".to_string(),
                ..ParentRef::default()
            }],
            rules: vec![HttpRule {
                name: String::new(),
                matches: vec![],
                filters: vec![],
                backend_refs: vec![BackendRef {
                    namespace: "gateway-conformance-mesh".to_string(),
                    name: "echo-v1".to_string(),
                    port: 80,
                    ..BackendRef::default()
                }],
                timeouts: None,
                retry: None,
                session_persistence: None,
            }],
            annotations: BTreeMap::new(),
        }],
        backends: vec![BackendCluster {
            name: "echo-v1:80".to_string(),
            namespace: "gateway-conformance-mesh".to_string(),
            protocol: "HTTP".to_string(),
            endpoints: vec![BackendEndpoint {
                address: "10.0.0.11".to_string(),
                port: 8080,
                healthy: true,
            }],
            wasm_plugin: None,
                ai_service: None,
                token_policy: None,
        }],
        workloads: vec![Workload {
            namespace: "gateway-conformance-mesh".to_string(),
            name: "producer".to_string(),
            ip: "10.1.0.20".to_string(),
        }],
        ..Snapshot::default()
    };

    let mut request = RequestMeta::with_port(
        Some("echo-v1.gateway-conformance-mesh".to_string()),
        20080,
        "/",
        "GET",
        BTreeMap::new(),
    );
    request.source_ip = Some("10.1.0.20".to_string());

    let selected = snapshot.select_backend(&request).expect("mesh backend");

    assert_eq!(selected.backend_name, "gateway-conformance-mesh/echo-v1:80");
    assert_eq!(selected.backend.port, 8080);
}
