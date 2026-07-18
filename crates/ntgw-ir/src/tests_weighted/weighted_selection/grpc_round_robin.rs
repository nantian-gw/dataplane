#[test]
fn selects_grpc_backend_refs_by_weighted_round_robin() {
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
                    weighted_backend_ref("default", "users", 7070, 1),
                    weighted_backend_ref("default", "orders", 7071, 3),
                ],
                session_persistence: None,
            }],
            labels: BTreeMap::new(),
            annotations: BTreeMap::new(),
        }],
        backends: vec![
            BackendCluster {
                name: "users:7070".to_string().into(),
                namespace: "default".to_string().into(),
                protocol: "GRPC".to_string().into(),
                endpoints: vec![BackendEndpoint {
                    address: "10.0.0.10".to_string(),
                    port: 7070,
                    healthy: true,
                }],
                wasm_plugin: None,
                ai_service: None,
                token_policy: None,
            
                circuit_breaker: None,},
            BackendCluster {
                name: "orders:7071".to_string().into(),
                namespace: "default".to_string().into(),
                protocol: "GRPC".to_string().into(),
                endpoints: vec![BackendEndpoint {
                    address: "10.0.0.11".to_string(),
                    port: 7071,
                    healthy: true,
                }],
                wasm_plugin: None,
                ai_service: None,
                token_policy: None,
            
                circuit_breaker: None,},
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

    let selected = (0..8)
        .map(|_| {
            snapshot
                .select_grpc_backend(&request)
                .expect("selected grpc backend")
                .backend_name
        })
        .collect::<Vec<_>>();

    assert_eq!(
        selected,
        vec![
            "default/users:7070",
            "default/orders:7071",
            "default/orders:7071",
            "default/orders:7071",
            "default/users:7070",
            "default/orders:7071",
            "default/orders:7071",
            "default/orders:7071",
        ]
    );
}
