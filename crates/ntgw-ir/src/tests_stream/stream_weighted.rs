#[test]
fn selects_stream_backend_refs_by_weighted_round_robin() {
    let snapshot = Snapshot {
        listeners: vec![Listener {
            name: "default/gw/tcp".to_string(),
            address: "0.0.0.0".to_string(),
            addresses: vec!["0.0.0.0".to_string()],
            port: 9000,
            protocol: "LISTENER_PROTOCOL_TCP".to_string(),
            hostnames: vec![],
            attached_routes: vec!["default/tcp-route".to_string()],
            tls: None,
            backend_tls: None,
            metadata: BTreeMap::new(),
        }],
        stream_routes: vec![StreamRoute {
            name: "tcp-route".to_string(),
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
                backend_refs: vec![
                    weighted_backend_ref("default", "tcp-a", 9001, 1),
                    weighted_backend_ref("default", "tcp-b", 9002, 2),
                ],
            }],
            labels: BTreeMap::new(),
            annotations: BTreeMap::new(),
        }],
        backends: vec![
            BackendCluster {
                name: "tcp-a:9001".to_string(),
                namespace: "default".to_string(),
                protocol: "TCP".to_string(),
                endpoints: vec![BackendEndpoint {
                    address: "10.0.0.40".to_string(),
                    port: 9001,
                    healthy: true,
                }],
                wasm_plugin: None,
                ai_service: None,
                token_policy: None,

                circuit_breaker: None,

                security_policy: None,

                },
            BackendCluster {
                name: "tcp-b:9002".to_string(),
                namespace: "default".to_string(),
                protocol: "TCP".to_string(),
                endpoints: vec![BackendEndpoint {
                    address: "10.0.0.41".to_string(),
                    port: 9002,
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

    let selected = collect_stream_backends(&snapshot, "default/gw/tcp", None, 6);

    assert_eq!(
        selected,
        vec![
            "default/tcp-a:9001",
            "default/tcp-b:9002",
            "default/tcp-b:9002",
            "default/tcp-a:9001",
            "default/tcp-b:9002",
            "default/tcp-b:9002",
        ]
    );
}
