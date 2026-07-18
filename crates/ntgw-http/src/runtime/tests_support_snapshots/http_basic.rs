fn simple_http_snapshot(
    listener_port: u16,
    path: &str,
    backend_port: u32,
    protocol: &str,
) -> ntgw_ir::SharedSnapshot {
    let shared = Snapshot::shared();
    shared.store(Arc::new(Snapshot {
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
                backend_refs: vec![BackendRef {
                    namespace: "default".to_string(),
                    name: "backend".to_string(),
                    port: backend_port,
                    ..BackendRef::default()
                }],
                ..HttpRule::default()
            }],
            labels: BTreeMap::new(),
            annotations: BTreeMap::new(),
        }],
        backends: vec![BackendCluster {
            ai_service: None,
            token_policy: None,
            name: format!("backend:{backend_port}").into(),
            namespace: "default".to_string().into(),
            protocol: protocol.to_string().into(),
            endpoints: vec![BackendEndpoint {
                address: "127.0.0.1".to_string(),
                port: backend_port,
                healthy: true,
            }],
            wasm_plugin: None,
        
                circuit_breaker: None,}],
        ..Snapshot::default()
    }));
    {
        let mut s = (**shared.load()).clone();
        s.rebuild_runtime_indexes();
        shared.store(Arc::new(s));
    }
    shared
}

fn cors_http_snapshot(
    listener_port: u16,
    path: &str,
    backend_port: u32,
) -> ntgw_ir::SharedSnapshot {
    let shared = Snapshot::shared();
    shared.store(Arc::new(Snapshot {
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
                filters: vec![Filter {
                    filter_type: "CORS".to_string(),
                    cors: Some(CorsFilter {
                        allow_origins: vec!["https://app.example".to_string()],
                        allow_methods: vec!["GET".to_string(), "POST".to_string()],
                        allow_headers: vec![
                            "authorization".to_string(),
                            "content-type".to_string(),
                        ],
                        max_age: Some(600),
                        ..CorsFilter::default()
                    }),
                    ..Filter::default()
                }],
                backend_refs: vec![BackendRef {
                    namespace: "default".to_string(),
                    name: "backend".to_string(),
                    port: backend_port,
                    ..BackendRef::default()
                }],
                ..HttpRule::default()
            }],
            labels: BTreeMap::new(),
            annotations: BTreeMap::new(),
        }],
        backends: vec![BackendCluster {
            ai_service: None,
            token_policy: None,
            name: format!("backend:{backend_port}").into(),
            namespace: "default".to_string().into(),
            protocol: "HTTP".to_string().into(),
            endpoints: vec![BackendEndpoint {
                address: "127.0.0.1".to_string(),
                port: backend_port,
                healthy: true,
            }],
            wasm_plugin: None,
        
                circuit_breaker: None,}],
        ..Snapshot::default()
    }));
    {
        let mut s = (**shared.load()).clone();
        s.rebuild_runtime_indexes();
        shared.store(Arc::new(s));
    }
    shared
}

fn dual_protocol_snapshot(
    listener_port: u16,
    http_backend_port: u32,
    h2c_backend_port: u32,
) -> ntgw_ir::SharedSnapshot {
    let shared = Snapshot::shared();
    shared.store(Arc::new(Snapshot {
        listeners: vec![Listener {
            name: "default/gw/http".to_string(),
            address: "127.0.0.1".to_string(),
            addresses: vec!["127.0.0.1".to_string()],
            port: listener_port as u32,
            protocol: "LISTENER_PROTOCOL_HTTP".to_string(),
            attached_routes: vec![
                "default/http-route".to_string(),
                "default/h2c-route".to_string(),
            ],
            ..Listener::default()
        }],
        http_routes: vec![
            HttpRoute {
                name: "http-route".to_string(),
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
                        path: "/http".to_string(),
                        path_type: "Exact".to_string(),
                        ..HttpMatch::default()
                    }],
                    backend_refs: vec![BackendRef {
                        namespace: "default".to_string(),
                        name: "http-backend".to_string(),
                        port: http_backend_port,
                        ..BackendRef::default()
                    }],
                    ..HttpRule::default()
                }],
                labels: BTreeMap::new(),
                annotations: BTreeMap::new(),
            },
            HttpRoute {
                name: "h2c-route".to_string(),
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
                        path: "/h2c".to_string(),
                        path_type: "Exact".to_string(),
                        ..HttpMatch::default()
                    }],
                    backend_refs: vec![BackendRef {
                        namespace: "default".to_string(),
                        name: "h2c-backend".to_string(),
                        port: h2c_backend_port,
                        ..BackendRef::default()
                    }],
                    ..HttpRule::default()
                }],
                labels: BTreeMap::new(),
                annotations: BTreeMap::new(),
            },
        ],
        backends: vec![
            BackendCluster {
                ai_service: None,
                token_policy: None,
                name: format!("http-backend:{http_backend_port}").into(),
                namespace: "default".to_string().into(),
                protocol: "HTTP".to_string().into(),
                endpoints: vec![BackendEndpoint {
                    address: "127.0.0.1".to_string(),
                    port: http_backend_port,
                    healthy: true,
                }],
                wasm_plugin: None,
            
                circuit_breaker: None,},
            BackendCluster {
                ai_service: None,
                token_policy: None,
                name: format!("h2c-backend:{h2c_backend_port}").into(),
                namespace: "default".to_string().into(),
                protocol: "H2C".to_string().into(),
                endpoints: vec![BackendEndpoint {
                    address: "127.0.0.1".to_string(),
                    port: h2c_backend_port,
                    healthy: true,
                }],
                wasm_plugin: None,
            
                circuit_breaker: None,},
        ],
        ..Snapshot::default()
    }));
    {
        let mut s = (**shared.load()).clone();
        s.rebuild_runtime_indexes();
        shared.store(Arc::new(s));
    }
    shared
}
