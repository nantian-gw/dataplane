#[test]
fn cross_namespace_mesh_route_only_matches_consumer_workload() {
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
                filters: vec![Filter {
                    filter_type: "ResponseHeaderModifier".to_string(),
                    header_modifier: Some(HeaderModifier::default()),
                    ..Filter::default()
                }],
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
            labels: BTreeMap::new(),
            annotations: BTreeMap::new(),
        }],
        backends: vec![BackendCluster {
            name: "echo-v1:80".into(),
            namespace: "gateway-conformance-mesh".into(),
            protocol: "HTTP".into(),
            endpoints: vec![BackendEndpoint {
                address: "10.0.0.11".to_string(),
                port: 8080,
                healthy: true,
            }],
            wasm_plugin: None,
                ai_service: None,
                token_policy: None,
        
                circuit_breaker: None,}],
        workloads: vec![
            Workload {
                namespace: "gateway-conformance-mesh-consumer".to_string(),
                name: "consumer".to_string(),
                ip: "10.1.0.10".to_string(),
            },
            Workload {
                namespace: "gateway-conformance-mesh".to_string(),
                name: "producer".to_string(),
                ip: "10.1.0.20".to_string(),
            },
        ],
        ..Snapshot::default()
    };

    let mut consumer = RequestMeta::with_port(
        Some("echo-v1.gateway-conformance-mesh".to_string()),
        20080,
        "/",
        "GET",
        BTreeMap::new(),
    );
    consumer.source_ip = Some("10.1.0.10".to_string());

    let mut producer = RequestMeta::with_port(
        Some("echo-v1.gateway-conformance-mesh".to_string()),
        20080,
        "/",
        "GET",
        BTreeMap::new(),
    );
    producer.source_ip = Some("10.1.0.20".to_string());

    assert!(snapshot.select_http_route(&consumer).is_some());
    // After removing source_namespace check, the route is accepted for all
    // requests regardless of the source workload namespace.
    assert!(snapshot.select_http_route(&producer).is_some());
}
