fn multi_backend_http_snapshot(
    listener_port: u16,
    path: &str,
    backends: &[(&str, u32, &str, u32)],
    retry: Option<RetryPolicy>,
) -> aeg_ir::SharedSnapshot {
    let shared = Snapshot::shared();
    *shared.write() = Snapshot {
        listeners: vec![Listener {
            name: "default/gw/http".to_string(),
            address: "127.0.0.1".to_string(),
            addresses: vec!["127.0.0.1".to_string()],
            port: listener_port as u32,
            protocol: "LISTENER_PROTOCOL_HTTP".to_string(),
            attached_routes: vec!["default/route".to_string()],
            ..Listener::default()
        }],
        http_routes: vec![HttpRoute {
            name: "route".to_string(),
            namespace: "default".to_string(),
            hostnames: Vec::new(),
            parent_refs: vec![ParentRef {
                namespace: "default".to_string(),
                name: "gw".to_string(),
                section_name: String::new(),
                port: listener_port as u32,
                ..ParentRef::default()
            }],
            rules: vec![HttpRule {
                name: String::new(),
                matches: vec![HttpMatch {
                    path: path.to_string(),
                    path_type: "Exact".to_string(),
                    ..HttpMatch::default()
                }],
                backend_refs: backends
                    .iter()
                    .map(|(name, port, _, weight)| BackendRef {
                        namespace: "default".to_string(),
                        name: (*name).to_string(),
                        port: *port,
                        weight: *weight,
                        ..BackendRef::default()
                    })
                    .collect(),
                retry,
                ..HttpRule::default()
            }],
            annotations: BTreeMap::new(),
        }],
        backends: backends
            .iter()
            .map(|(name, port, protocol, _)| BackendCluster {
                ai_service: None,
                token_policy: None,
                name: format!("{name}:{port}"),
                namespace: "default".to_string(),
                protocol: (*protocol).to_string(),
                endpoints: vec![BackendEndpoint {
                    address: "127.0.0.1".to_string(),
                    port: *port,
                    healthy: true,
                }],
                wasm_plugin: None,
            })
            .collect(),
        ..Snapshot::default()
    };
    shared.write().rebuild_runtime_indexes();
    shared
}
