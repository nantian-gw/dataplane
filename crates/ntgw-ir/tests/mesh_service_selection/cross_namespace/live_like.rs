#[test]
fn live_like_cross_namespace_mesh_route_matches_consumer_on_service_frontend() {
    let snapshot = Snapshot {
        listeners: vec![
            mesh_listener(
                "nantian-mesh-validation",
                "echo-v1",
                80,
                25064,
                "HTTP",
                &["nantian-mesh-consumer-validation/mesh-consumer-route"],
            ),
            mesh_listener(
                "nantian-mesh-validation",
                "echo-v1",
                8080,
                24688,
                "HTTP",
                &["nantian-mesh-consumer-validation/mesh-consumer-route"],
            ),
            mesh_listener(
                "nantian-mesh-validation",
                "echo-v1",
                7070,
                21464,
                "GRPC",
                &[],
            ),
            mesh_listener("nantian-mesh-validation", "echo-v2", 80, 26903, "HTTP", &[]),
            mesh_listener(
                "nantian-mesh-validation",
                "echo-v2",
                8080,
                29639,
                "HTTP",
                &[],
            ),
            mesh_listener(
                "nantian-mesh-validation",
                "echo-v2",
                7070,
                22463,
                "GRPC",
                &[],
            ),
        ],
        http_routes: vec![HttpRoute {
            name: "mesh-consumer-route".to_string(),
            namespace: "nantian-mesh-consumer-validation".to_string(),
            hostnames: vec![],
            parent_refs: vec![ParentRef {
                kind: "Service".to_string(),
                namespace: "nantian-mesh-validation".to_string(),
                name: "echo-v1".to_string(),
                ..ParentRef::default()
            }],
            rules: vec![HttpRule {
                name: String::new(),
                matches: vec![HttpMatch {
                    path: "/".to_string(),
                    path_type: "PathPrefix".to_string(),
                    ..HttpMatch::default()
                }],
                filters: vec![Filter {
                    filter_type: "ResponseHeaderModifier".to_string(),
                    header_modifier: Some(HeaderModifier::default()),
                    ..Filter::default()
                }],
                backend_refs: vec![BackendRef {
                    namespace: "nantian-mesh-validation".to_string(),
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
        backends: vec![
            BackendCluster {
                name: "echo-v1:80".to_string(),
                namespace: "nantian-mesh-validation".to_string(),
                protocol: "HTTP".to_string(),
                endpoints: vec![BackendEndpoint {
                    address: "10.244.0.155".to_string(),
                    port: 8080,
                    healthy: true,
                }],
                wasm_plugin: None,
                ai_service: None,
                token_policy: None,
            },
            BackendCluster {
                name: "echo-v1:8080".to_string(),
                namespace: "nantian-mesh-validation".to_string(),
                protocol: "HTTP".to_string(),
                endpoints: vec![BackendEndpoint {
                    address: "10.244.0.155".to_string(),
                    port: 8080,
                    healthy: true,
                }],
                wasm_plugin: None,
                ai_service: None,
                token_policy: None,
            },
            BackendCluster {
                name: "echo-v2:80".to_string(),
                namespace: "nantian-mesh-validation".to_string(),
                protocol: "HTTP".to_string(),
                endpoints: vec![BackendEndpoint {
                    address: "10.244.0.156".to_string(),
                    port: 8080,
                    healthy: true,
                }],
                wasm_plugin: None,
                ai_service: None,
                token_policy: None,
            },
        ],
        workloads: vec![
            Workload {
                namespace: "nantian-mesh-consumer-validation".to_string(),
                name: "consumer".to_string(),
                ip: "10.244.0.158".to_string(),
            },
            Workload {
                namespace: "nantian-mesh-validation".to_string(),
                name: "producer".to_string(),
                ip: "10.244.0.157".to_string(),
            },
        ],
        ..Snapshot::default()
    };

    let mut consumer = RequestMeta::with_port(
        Some("echo-v1.nantian-mesh-validation".to_string()),
        25064,
        "/",
        "GET",
        BTreeMap::new(),
    );
    consumer.source_ip = Some("10.244.0.158".to_string());

    let mut producer = RequestMeta::with_port(
        Some("echo-v1.nantian-mesh-validation".to_string()),
        25064,
        "/",
        "GET",
        BTreeMap::new(),
    );
    producer.source_ip = Some("10.244.0.157".to_string());

    let selected = snapshot
        .select_http_route(&consumer)
        .expect("mesh consumer route");
    assert_eq!(selected.route_name, "mesh-consumer-route");
    assert_eq!(selected.route_namespace, "nantian-mesh-consumer-validation");

    assert!(snapshot.select_http_route(&producer).is_none());
}
