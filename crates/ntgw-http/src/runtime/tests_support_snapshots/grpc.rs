fn grpc_h2c_snapshot(listener_port: u16, backend_port: u32) -> ntgw_ir::SharedSnapshot {
    let shared = Snapshot::shared();
    shared.store(Arc::new(Snapshot {
        listeners: vec![Listener {
            name: "default/gw/http".to_string().into(),
            address: "127.0.0.1".to_string(),
            addresses: vec!["127.0.0.1".to_string()],
            port: listener_port as u32,
            protocol: "LISTENER_PROTOCOL_HTTP".to_string().into(),
            attached_routes: vec!["default/grpc-route".to_string()],
            ..Listener::default()
        }],
        grpc_routes: vec![GrpcRoute {
            name: "grpc-route".to_string().into(),
            namespace: "default".to_string().into(),
            hostnames: Vec::new(),
            parent_refs: vec![ParentRef {
                namespace: "default".to_string().into(),
                name: "gw".to_string().into(),
                section_name: String::new(),
                port: listener_port as u32,
                ..ParentRef::default()
            }],
            rules: vec![GrpcRule {
                name: String::new(),
                matches: vec![GrpcMatch {
                    service: "helloworld.Greeter".to_string(),
                    method: "Watch".to_string(),
                    ..GrpcMatch::default()
                }],
                backend_refs: vec![BackendRef {
                    namespace: "default".to_string().into(),
                    name: "grpc-backend".to_string().into(),
                    port: backend_port,
                    ..BackendRef::default()
                }],
                ..GrpcRule::default()
            }],
            labels: BTreeMap::new(),
            annotations: BTreeMap::new(),
        }],
        backends: vec![BackendCluster {
            ai_service: None,
            token_policy: None,
            name: format!("grpc-backend:{backend_port}").into(),
            namespace: "default".to_string().into(),
            protocol: "GRPC".to_string().into(),
            endpoints: vec![BackendEndpoint {
                address: "127.0.0.1".to_string(),
                port: backend_port,
                healthy: true,
            }],
            wasm_plugin: None,
        
                circuit_breaker: None,}],
        ..Snapshot::default()
    }));
    let mut s = (**shared.load()).clone();
    s.rebuild_runtime_indexes();
    shared.store(Arc::new(s));
    shared
}
