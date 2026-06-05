#[test]
fn spreads_large_weighted_grpc_backends_across_short_request_batches() {
    let snapshot = Snapshot {
        listeners: vec![Listener {
            name: "mesh/default/echo/26060".to_string(),
            address: "0.0.0.0".to_string(),
            addresses: vec!["0.0.0.0".to_string()],
            port: 26060,
            protocol: "LISTENER_PROTOCOL_GRPC".to_string(),
            hostnames: vec![],
            attached_routes: vec!["default/weighted".to_string()],
            tls: None,
            backend_tls: None,
            metadata: BTreeMap::new(),
        }],
        grpc_routes: vec![GrpcRoute {
            name: "weighted".to_string(),
            namespace: "default".to_string(),
            hostnames: vec![],
            parent_refs: vec![],
            rules: vec![GrpcRule {
                name: String::new(),
                matches: vec![],
                filters: vec![],
                backend_refs: vec![
                    weighted_backend_ref("default", "echo-v1", 7070, 70),
                    weighted_backend_ref("default", "echo-v2", 7070, 30),
                ],
                session_persistence: None,
            }],
            annotations: BTreeMap::new(),
        }],
        backends: vec![
            BackendCluster {
                name: "echo-v1:7070".to_string(),
                namespace: "default".to_string(),
                protocol: "GRPC".to_string(),
                endpoints: vec![BackendEndpoint {
                    address: "10.0.0.10".to_string(),
                    port: 7070,
                    healthy: true,
                }],
                wasm_plugin: None,
                ai_service: None,
                token_policy: None,
            },
            BackendCluster {
                name: "echo-v2:7070".to_string(),
                namespace: "default".to_string(),
                protocol: "GRPC".to_string(),
                endpoints: vec![BackendEndpoint {
                    address: "10.0.0.11".to_string(),
                    port: 7070,
                    healthy: true,
                }],
                wasm_plugin: None,
                ai_service: None,
                token_policy: None,
            },
        ],
        ..Snapshot::default()
    };
    let request = RequestMeta {
        host: Some("echo.default.svc.cluster.local".to_string()),
        port: 26060,
        path: "/grpcecho.Echo/Echo".to_string(),
        method: "POST".to_string(),
        source_ip: None,
        headers: BTreeMap::from([(
            "content-type".to_string(),
            vec!["application/grpc".to_string()],
        )]),
        query_params: BTreeMap::new(),
    };

    let selected = (0..20)
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

    assert_eq!(v1, 14, "selected={selected:?}");
    assert_eq!(v2, 6, "selected={selected:?}");
}
