fn disabled_access_log() -> AccessLogOptions {
    AccessLogOptions {
        enabled: false,
        ..AccessLogOptions::default()
    }
}

fn test_listener(name: &str, port: u32) -> Listener {
    Listener {
        name: name.to_string(),
        address: "127.0.0.1".to_string(),
        addresses: vec!["127.0.0.1".to_string()],
        port,
        protocol: "LISTENER_PROTOCOL_UDP".to_string(),
        hostnames: Vec::new(),
        attached_routes: vec!["default/udp-route".to_string()],
        tls: None,
        backend_tls: None,
        metadata: BTreeMap::new(),
    }
security_policy: None,
}

fn test_snapshot(listener: Listener, upstream_addr: std::net::SocketAddr) -> SharedSnapshot {
    let shared = Snapshot::shared();
    shared.store(Arc::new(Snapshot {
        listeners: vec![listener],
        stream_routes: vec![StreamRoute {
            name: "udp-route".to_string(),
            namespace: "default".to_string(),
            kind: "ROUTE_KIND_UDP".to_string(),
            parent_refs: Vec::new(),
            rules: vec![StreamRule {
                name: String::new(),
                matches: vec![StreamMatch::default()],
                backend_refs: vec![BackendRef {
                    namespace: "default".to_string(),
                    name: "dns".to_string(),
                    port: upstream_addr.port() as u32,
                    ..BackendRef::default()
                }],
            }],
            labels: BTreeMap::new(),
            annotations: BTreeMap::new(),
        }],
        security_policy: None,
        backends: vec![BackendCluster {
            ai_service: None,
            token_policy: None,
            name: format!("dns:{}", upstream_addr.port()).into(),
            namespace: "default".to_string().into(),
            protocol: "UDP".to_string().into(),
            endpoints: vec![BackendEndpoint {
                address: upstream_addr.ip().to_string(),
                port: upstream_addr.port() as u32,
                healthy: true,
            }],
            wasm_plugin: None,
        
                circuit_breaker: None,}],
        ..Snapshot::default()
    }));
    shared
}
