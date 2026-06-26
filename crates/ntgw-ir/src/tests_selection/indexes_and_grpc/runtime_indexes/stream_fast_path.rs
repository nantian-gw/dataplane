#[test]
fn stream_fast_path_precompiles_backend_refs_for_attached_routes() {
    let mut snapshot = Snapshot {
        listeners: vec![Listener {
            name: "default/gw/tcp".to_string(),
            port: 9000,
            protocol: "TCP".to_string(),
            attached_routes: vec!["default/tcp-route".to_string()],
            ..Listener::default()
        }],
        stream_routes: vec![StreamRoute {
            name: "tcp-route".to_string(),
            namespace: "default".to_string(),
            kind: "ROUTE_KIND_TCP".to_string(),
            rules: vec![StreamRule {
                matches: vec![StreamMatch {
                    port: 9000,
                    ..StreamMatch::default()
                }],
                backend_refs: vec![backend_ref("default", "orders", 8080)],
                ..StreamRule::default()
            }],
            ..StreamRoute::default()
        }],
        backends: vec![BackendCluster {
            name: "orders:8080".to_string(),
            namespace: "default".to_string(),
            protocol: "TCP".to_string(),
            endpoints: vec![BackendEndpoint {
                address: "10.0.0.10".to_string(),
                port: 8080,
                healthy: true,
            }],
            wasm_plugin: None,
                ai_service: None,
                token_policy: None,
        
                circuit_breaker: None,}],
        ..Snapshot::default()
    };

    snapshot.rebuild_runtime_indexes();

    assert_eq!(snapshot.stream_fast_path.route_count(), 1);
    assert_eq!(snapshot.stream_fast_path.eligible_rule_count(), 1);
    assert_eq!(snapshot.stream_fast_path.compiled_backend_ref_count(), 1);

    let selected = snapshot
        .select_stream_backend("default/gw/tcp", None)
        .expect("stream fast path should select backend");
    assert_eq!(selected.route_kind, RouteKind::Tcp);
    assert_eq!(selected.route_name, "tcp-route");
    assert_eq!(selected.backend_name, "default/orders:8080");
    assert_eq!(selected.backend.address, "10.0.0.10");
}

#[test]
fn stream_fast_path_skips_unavailable_compiled_backend_and_keeps_searching() {
    let mut snapshot = Snapshot {
        listeners: vec![Listener {
            name: "default/gw/tcp".to_string(),
            port: 9000,
            protocol: "TCP".to_string(),
            attached_routes: vec![
                "default/unavailable".to_string(),
                "default/available".to_string(),
            ],
            ..Listener::default()
        }],
        stream_routes: vec![
            StreamRoute {
                name: "unavailable".to_string(),
                namespace: "default".to_string(),
                kind: "ROUTE_KIND_TCP".to_string(),
                rules: vec![StreamRule {
                    matches: vec![StreamMatch {
                        port: 9000,
                        ..StreamMatch::default()
                    }],
                    backend_refs: vec![backend_ref("default", "unavailable", 8080)],
                    ..StreamRule::default()
                }],
                ..StreamRoute::default()
            },
            StreamRoute {
                name: "available".to_string(),
                namespace: "default".to_string(),
                kind: "ROUTE_KIND_TCP".to_string(),
                rules: vec![StreamRule {
                    matches: vec![StreamMatch {
                        port: 9000,
                        ..StreamMatch::default()
                    }],
                    backend_refs: vec![backend_ref("default", "available", 8080)],
                    ..StreamRule::default()
                }],
                ..StreamRoute::default()
            },
        ],
        backends: vec![
            BackendCluster {
                name: "unavailable:8080".to_string(),
                namespace: "default".to_string(),
                protocol: "TCP".to_string(),
                endpoints: vec![BackendEndpoint {
                    address: "10.0.0.10".to_string(),
                    port: 8080,
                    healthy: false,
                }],
                wasm_plugin: None,
                ai_service: None,
                token_policy: None,
            
                circuit_breaker: None,},
            BackendCluster {
                name: "available:8080".to_string(),
                namespace: "default".to_string(),
                protocol: "TCP".to_string(),
                endpoints: vec![BackendEndpoint {
                    address: "10.0.0.20".to_string(),
                    port: 8080,
                    healthy: true,
                }],
                wasm_plugin: None,
                ai_service: None,
                token_policy: None,
            
                circuit_breaker: None,},
        ],
        ..Snapshot::default()
    };

    snapshot.rebuild_runtime_indexes();

    let selected = snapshot
        .select_stream_backend("default/gw/tcp", None)
        .expect("stream fast path should keep searching after an unavailable backend");
    assert_eq!(selected.route_name, "available");
    assert_eq!(selected.backend_name, "default/available:8080");
    assert_eq!(selected.backend.address, "10.0.0.20");
}

#[test]
fn stream_fast_path_falls_back_to_default_service_backend_when_no_route_matches() {
    let mut snapshot = Snapshot {
        listeners: vec![Listener {
            name: "default/gw/tcp".to_string(),
            port: 9000,
            protocol: "TCP".to_string(),
            metadata: service_frontend_metadata("default", "orders", 8080),
            ..Listener::default()
        }],
        backends: vec![BackendCluster {
            name: "orders:8080".to_string(),
            namespace: "default".to_string(),
            protocol: "TCP".to_string(),
            endpoints: vec![BackendEndpoint {
                address: "10.0.0.30".to_string(),
                port: 8080,
                healthy: true,
            }],
            wasm_plugin: None,
                ai_service: None,
                token_policy: None,
        
                circuit_breaker: None,}],
        ..Snapshot::default()
    };

    snapshot.rebuild_runtime_indexes();

    let selected = snapshot
        .select_stream_backend("default/gw/tcp", None)
        .expect("stream fast path should fall back to the default service backend");
    assert_eq!(selected.route_name, "");
    assert_eq!(selected.route_namespace, "default");
    assert_eq!(selected.backend_name, "default/orders:8080");
    assert_eq!(selected.backend.address, "10.0.0.30");
}
