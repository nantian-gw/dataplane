fn grpc_h2c_snapshot(listener_port: u16, backend_port: u32) -> aeg_ir::SharedSnapshot {
    let shared = Snapshot::shared();
    *shared.write() = Snapshot {
        listeners: vec![Listener {
            name: "default/gw/http".to_string(),
            address: "127.0.0.1".to_string(),
            addresses: vec!["127.0.0.1".to_string()],
            port: listener_port as u32,
            protocol: "LISTENER_PROTOCOL_HTTP".to_string(),
            attached_routes: vec!["default/grpc-route".to_string()],
            ..Listener::default()
        }],
        grpc_routes: vec![GrpcRoute {
            name: "grpc-route".to_string(),
            namespace: "default".to_string(),
            hostnames: Vec::new(),
            parent_refs: vec![ParentRef {
                namespace: "default".to_string(),
                name: "gw".to_string(),
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
                    namespace: "default".to_string(),
                    name: "grpc-backend".to_string(),
                    port: backend_port,
                    ..BackendRef::default()
                }],
                ..GrpcRule::default()
            }],
            annotations: BTreeMap::new(),
        }],
        backends: vec![BackendCluster {
            ai_service: None,
            token_policy: None,
            name: format!("grpc-backend:{backend_port}"),
            namespace: "default".to_string(),
            protocol: "GRPC".to_string(),
            endpoints: vec![BackendEndpoint {
                address: "127.0.0.1".to_string(),
                port: backend_port,
                healthy: true,
            }],
            wasm_plugin: None,
        }],
        ..Snapshot::default()
    };
    shared.write().rebuild_runtime_indexes();
    shared
}
