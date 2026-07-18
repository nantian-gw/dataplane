use super::*;

#[test]
fn mesh_grpc_service_frontend_weighted_backends_keep_expected_distribution() {
    let mut headers = BTreeMap::new();
    headers.insert(
        "content-type".to_string(),
        vec!["application/grpc".to_string()],
    );

    let snapshot = Snapshot {
        listeners: vec![mesh_listener(
            "default",
            "echo",
            7070,
            27070,
            "GRPC",
            &["default/mesh-grpc-weighted-backends"],
        )],
        grpc_routes: vec![GrpcRoute {
            name: "mesh-grpc-weighted-backends".to_string(),
            namespace: "default".to_string(),
            hostnames: vec![],
            parent_refs: vec![ParentRef {
                kind: "Service".to_string(),
                name: "echo".to_string(),
                port: 7070,
                ..ParentRef::default()
            }],
            rules: vec![GrpcRule {
                name: String::new(),
                matches: vec![],
                filters: vec![],
                backend_refs: vec![
                    BackendRef {
                        namespace: "default".to_string(),
                        name: "echo-v1".to_string(),
                        port: 7070,
                        weight: 70,
                        ..BackendRef::default()
                    },
                    BackendRef {
                        namespace: "default".to_string(),
                        name: "echo-v2".to_string(),
                        port: 7070,
                        weight: 30,
                        ..BackendRef::default()
                    },
                ],
                session_persistence: None,
            }],
            labels: BTreeMap::new(),
            annotations: BTreeMap::new(),
        }],
        backends: vec![
            BackendCluster {
                name: "echo-v1:7070".into(),
                namespace: "default".into(),
                protocol: "GRPC".into(),
                endpoints: vec![BackendEndpoint {
                    address: "10.0.0.10".to_string(),
                    port: 7070,
                    healthy: true,
                }],
                wasm_plugin: None,
                ai_service: None,
                token_policy: None,

                circuit_breaker: None,
            },
            BackendCluster {
                name: "echo-v2:7070".into(),
                namespace: "default".into(),
                protocol: "GRPC".into(),
                endpoints: vec![BackendEndpoint {
                    address: "10.0.0.11".to_string(),
                    port: 7070,
                    healthy: true,
                }],
                wasm_plugin: None,
                ai_service: None,
                token_policy: None,

                circuit_breaker: None,
            },
        ],
        ..Snapshot::default()
    };

    let request = RequestMeta::with_port(
        Some("echo".to_string()),
        27070,
        "/grpcecho.Echo/Ping",
        "POST",
        headers,
    );

    let selected = (0..100)
        .map(|_| {
            snapshot
                .select_grpc_backend(&request)
                .expect("selected grpc backend")
                .backend_name
        })
        .collect::<Vec<_>>();

    let v1 = selected
        .iter()
        .filter(|backend| backend.as_str() == "default/echo-v1:7070")
        .count();
    let v2 = selected
        .iter()
        .filter(|backend| backend.as_str() == "default/echo-v2:7070")
        .count();

    assert_eq!(v1, 70, "selected={selected:?}");
    assert_eq!(v2, 30, "selected={selected:?}");
}
