#[test]
fn selects_tcp_backend_by_listener_port_isolation() {
    let snapshot = Snapshot {
        listeners: vec![
            Listener {
                name: "default/gw/tcp-9000".to_string(),
                address: "0.0.0.0".to_string(),
                addresses: vec!["0.0.0.0".to_string()],
                port: 9000,
                protocol: "LISTENER_PROTOCOL_TCP".to_string(),
                hostnames: vec![],
                attached_routes: vec!["default/tcp-a".to_string()],
                tls: None,
                backend_tls: None,
                metadata: BTreeMap::new(),
            },
            Listener {
                name: "default/gw/tcp-9001".to_string(),
                address: "0.0.0.0".to_string(),
                addresses: vec!["0.0.0.0".to_string()],
                port: 9001,
                protocol: "LISTENER_PROTOCOL_TCP".to_string(),
                hostnames: vec![],
                attached_routes: vec!["default/tcp-b".to_string()],
                tls: None,
                backend_tls: None,
                metadata: BTreeMap::new(),
            },
        ],
        stream_routes: vec![
            StreamRoute {
                name: "tcp-a".to_string(),
                namespace: "default".to_string(),
                kind: "ROUTE_KIND_TCP".to_string(),
                parent_refs: vec![],
                rules: vec![StreamRule {
                    name: String::new(),
                    matches: vec![StreamMatch {
                        port: 9000,
                        sni_hostname: String::new(),
                    mode: TlsRouteMode::default(),
                    }],
                    backend_refs: vec![backend_ref("default", "tcp-a", 7000)],
                }],
                labels: BTreeMap::new(),
            annotations: BTreeMap::new(),
            },
            StreamRoute {
                name: "tcp-b".to_string(),
                namespace: "default".to_string(),
                kind: "ROUTE_KIND_TCP".to_string(),
                parent_refs: vec![],
                rules: vec![StreamRule {
                    name: String::new(),
                    matches: vec![StreamMatch {
                        port: 9001,
                        sni_hostname: String::new(),
                    mode: TlsRouteMode::default(),
                    }],
                    backend_refs: vec![backend_ref("default", "tcp-b", 7001)],
                }],
                labels: BTreeMap::new(),
            annotations: BTreeMap::new(),
            },
        ],
        backends: vec![
            BackendCluster {
                name: "tcp-a:7000".to_string().into(),
                namespace: "default".to_string().into(),
                protocol: "TCP".to_string().into(),
                endpoints: vec![BackendEndpoint {
                    address: "10.0.0.50".to_string(),
                    port: 7000,
                    healthy: true,
                }],
                wasm_plugin: None,
                ai_service: None,
                token_policy: None,

                circuit_breaker: None,

                security_policy: None,

                },
            BackendCluster {
                name: "tcp-b:7001".to_string().into(),
                namespace: "default".to_string().into(),
                protocol: "TCP".to_string().into(),
                endpoints: vec![BackendEndpoint {
                    address: "10.0.0.51".to_string(),
                    port: 7001,
                    healthy: true,
                }],
                wasm_plugin: None,
                ai_service: None,
                token_policy: None,

                circuit_breaker: None,

                security_policy: None,

                },
        ],
        ..Snapshot::default()
    };

    let first = snapshot
        .select_stream_backend("default/gw/tcp-9000", None)
        .expect("tcp-9000 backend");
    let second = snapshot
        .select_stream_backend("default/gw/tcp-9001", None)
        .expect("tcp-9001 backend");

    assert_eq!(first.route_name, "tcp-a");
    assert_eq!(first.backend.address, "10.0.0.50");
    assert_eq!(second.route_name, "tcp-b");
    assert_eq!(second.backend.address, "10.0.0.51");
}
