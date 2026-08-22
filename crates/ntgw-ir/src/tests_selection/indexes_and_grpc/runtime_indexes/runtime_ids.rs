#[test]
fn runtime_indexes_assign_stable_runtime_ids_across_snapshot_order_changes() {
    let mut first = runtime_id_test_snapshot(false);
    let mut reordered = runtime_id_test_snapshot(true);

    first.rebuild_runtime_indexes();
    reordered.rebuild_runtime_indexes();

    assert_eq!(
        first.listener_runtime_id("default/gw/http"),
        reordered.listener_runtime_id("default/gw/http")
    );
    assert_ne!(
        first.listener_runtime_id("default/gw/http"),
        first.listener_runtime_id("default/gw/grpc")
    );
    assert_eq!(
        first.http_route_runtime_id("default", "orders"),
        reordered.http_route_runtime_id("default", "orders")
    );
    assert_eq!(
        first.grpc_route_runtime_id("default", "orders-grpc"),
        reordered.grpc_route_runtime_id("default", "orders-grpc")
    );
    assert_eq!(
        first.stream_route_runtime_id("TCPRoute", "default", "orders-tcp"),
        reordered.stream_route_runtime_id("TCPRoute", "default", "orders-tcp")
    );
    assert_eq!(
        first.http_rule_runtime_id("default", "orders", 0),
        reordered.http_rule_runtime_id("default", "orders", 0)
    );
    assert_ne!(
        first.http_rule_runtime_id("default", "orders", 0),
        first.http_rule_runtime_id("default", "orders", 1)
    );
    assert_eq!(
        first.backend_runtime_id("default/orders:8080"),
        reordered.backend_runtime_id("default/orders:8080")
    );
    assert_eq!(
        first.endpoint_runtime_id(
            "default/orders:8080",
            &BackendEndpoint {
                address: "10.0.0.10".to_string(),
                port: 8080,
                healthy: true,
            },
        ),
        reordered.endpoint_runtime_id(
            "default/orders:8080",
            &BackendEndpoint {
                address: "10.0.0.10".to_string(),
                port: 8080,
                healthy: true,
            },
        )
    );
    assert_ne!(
        first.endpoint_runtime_id(
            "default/orders:8080",
            &BackendEndpoint {
                address: "10.0.0.10".to_string(),
                port: 8080,
                healthy: true,
            },
        ),
        first.endpoint_runtime_id(
            "default/orders:8080",
            &BackendEndpoint {
                address: "10.0.0.11".to_string(),
                port: 8080,
                healthy: true,
            },
        )
    );
}

#[test]
fn selected_stream_backend_runtime_ids_normalize_proto_route_kind() {
    let mut snapshot = Snapshot {
        listeners: vec![Listener {
            name: "default/gw/tcp".to_string(),
            address: "0.0.0.0".to_string(),
            port: 9000,
            protocol: "LISTENER_PROTOCOL_TCP".to_string(),
            attached_routes: vec!["default/orders-tcp".to_string()],
            ..Listener::default()
        }],
        stream_routes: vec![StreamRoute {
            name: "orders-tcp".to_string(),
            namespace: "default".to_string(),
            kind: "ROUTE_KIND_TCP".to_string(),
            rules: vec![StreamRule {
                name: String::new(),
                matches: vec![StreamMatch {
                    port: 9000,
                    ..StreamMatch::default()
                }],
                backend_refs: vec![backend_ref("default", "orders", 8080)],
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

                circuit_breaker: None,

                security_policy: None,

                }],
        ..Snapshot::default()
    };
    snapshot.rebuild_runtime_indexes();

    let selected = snapshot
        .select_stream_backend("default/gw/tcp", None)
        .expect("selected stream backend");
    let ids = snapshot.selected_backend_runtime_ids(&selected);

    assert_eq!(
        ids.route,
        snapshot.stream_route_runtime_id("TCPRoute", "default", "orders-tcp")
    );
    assert_eq!(
        ids.rule,
        snapshot.stream_rule_runtime_id("TCPRoute", "default", "orders-tcp", 0)
    );
    assert!(ids.route.is_some());
    assert!(ids.rule.is_some());
}

#[test]
fn runtime_indexes_resolve_resource_refs_by_runtime_id() {
    let mut snapshot = runtime_id_test_snapshot(false);
    snapshot.rebuild_runtime_indexes();

    let listener_id = snapshot
        .listener_runtime_id("default/gw/http")
        .expect("listener runtime id");
    assert_eq!(
        snapshot.runtime_resource_ref(listener_id),
        Some(RuntimeResourceRef::Listener {
            name: "default/gw/http".to_string(),
        })
    );

    let http_route_id = snapshot
        .http_route_runtime_id("default", "orders")
        .expect("HTTPRoute runtime id");
    assert_eq!(
        snapshot.runtime_resource_ref(http_route_id),
        Some(RuntimeResourceRef::HttpRoute {
            namespace: "default".to_string(),
            name: "orders".to_string(),
        })
    );

    let http_rule_id = snapshot
        .http_rule_runtime_id("default", "orders", 1)
        .expect("HTTPRoute rule runtime id");
    assert_eq!(
        snapshot.runtime_resource_ref(http_rule_id),
        Some(RuntimeResourceRef::HttpRule {
            namespace: "default".to_string(),
            name: "orders".to_string(),
            rule_index: 1,
        })
    );

    let grpc_route_id = snapshot
        .grpc_route_runtime_id("default", "orders-grpc")
        .expect("GRPCRoute runtime id");
    assert_eq!(
        snapshot.runtime_resource_ref(grpc_route_id),
        Some(RuntimeResourceRef::GrpcRoute {
            namespace: "default".to_string(),
            name: "orders-grpc".to_string(),
        })
    );
    let grpc_rule_id = snapshot
        .grpc_rule_runtime_id("default", "orders-grpc", 0)
        .expect("GRPCRoute rule runtime id");
    assert_eq!(
        snapshot.runtime_resource_ref(grpc_rule_id),
        Some(RuntimeResourceRef::GrpcRule {
            namespace: "default".to_string(),
            name: "orders-grpc".to_string(),
            rule_index: 0,
        })
    );

    let stream_route_id = snapshot
        .stream_route_runtime_id("TCPRoute", "default", "orders-tcp")
        .expect("TCPRoute runtime id");
    assert_eq!(
        snapshot.runtime_resource_ref(stream_route_id),
        Some(RuntimeResourceRef::StreamRoute {
            kind: "TCPRoute".to_string(),
            namespace: "default".to_string(),
            name: "orders-tcp".to_string(),
        })
    );
    let stream_rule_id = snapshot
        .stream_rule_runtime_id("TCPRoute", "default", "orders-tcp", 0)
        .expect("TCPRoute rule runtime id");
    assert_eq!(
        snapshot.runtime_resource_ref(stream_rule_id),
        Some(RuntimeResourceRef::StreamRule {
            kind: "TCPRoute".to_string(),
            namespace: "default".to_string(),
            name: "orders-tcp".to_string(),
            rule_index: 0,
        })
    );

    let backend_id = snapshot
        .backend_runtime_id("default/orders:8080")
        .expect("backend runtime id");
    assert_eq!(
        snapshot.runtime_resource_ref(backend_id),
        Some(RuntimeResourceRef::Backend {
            name: "default/orders:8080".to_string(),
        })
    );

    let endpoint = BackendEndpoint {
        address: "10.0.0.10".to_string(),
        port: 8080,
        healthy: true,
    };
    let endpoint_id = snapshot
        .endpoint_runtime_id("default/orders:8080", &endpoint)
        .expect("endpoint runtime id");
    assert_eq!(
        snapshot.runtime_resource_ref(endpoint_id),
        Some(RuntimeResourceRef::Endpoint {
            backend_name: "default/orders:8080".to_string(),
            address: "10.0.0.10".to_string(),
            port: 8080,
        })
    );
}

fn runtime_id_test_snapshot(reordered: bool) -> Snapshot {
    let mut listeners = vec![
        Listener {
            name: "default/gw/http".to_string(),
            address: "0.0.0.0".to_string(),
            port: 80,
            protocol: "LISTENER_PROTOCOL_HTTP".to_string(),
            attached_routes: vec!["default/orders".to_string()],
            ..Listener::default()
        },
        Listener {
            name: "default/gw/grpc".to_string(),
            address: "0.0.0.0".to_string(),
            port: 8080,
            protocol: "LISTENER_PROTOCOL_GRPC".to_string(),
            attached_routes: vec!["default/orders-grpc".to_string()],
            ..Listener::default()
        },
    ];
    let mut backends = vec![
        BackendCluster {
            name: "orders:8080".to_string(),
            namespace: "default".to_string(),
            protocol: "HTTP".to_string(),
            endpoints: vec![
                BackendEndpoint {
                    address: "10.0.0.10".to_string(),
                    port: 8080,
                    healthy: true,
                },
                BackendEndpoint {
                    address: "10.0.0.11".to_string(),
                    port: 8080,
                    healthy: true,
                },
            ],
            wasm_plugin: None,
                ai_service: None,
                token_policy: None,

                circuit_breaker: None,

                security_policy: None,

                },
        BackendCluster {
            name: "payments:8080".to_string(),
            namespace: "default".to_string(),
            protocol: "HTTP".to_string(),
            endpoints: vec![BackendEndpoint {
                address: "10.0.0.20".to_string(),
                port: 8080,
                healthy: true,
            }],
            wasm_plugin: None,
                ai_service: None,
                token_policy: None,

                circuit_breaker: None,

                security_policy: None,

                },
    ];

    if reordered {
        listeners.reverse();
        backends.reverse();
        backends
            .iter_mut()
            .for_each(|backend| backend.endpoints.reverse());
    }

    Snapshot {
        listeners,
        http_routes: vec![HttpRoute {
            name: "orders".to_string(),
            namespace: "default".to_string(),
            hostnames: vec!["orders.example.com".to_string()],
            parent_refs: vec![],
            rules: vec![
                HttpRule {
                    name: String::new(),
                    matches: vec![HttpMatch {
                        path: "/".to_string(),
                        path_type: "PathPrefix".to_string(),
                        method: "GET".to_string(),
                        ..HttpMatch::default()
                    }],
                    backend_refs: vec![backend_ref("default", "orders", 8080)],
                    ..HttpRule::default()
                },
                HttpRule {
                    name: String::new(),
                    matches: vec![HttpMatch {
                        path: "/v2".to_string(),
                        path_type: "PathPrefix".to_string(),
                        method: "GET".to_string(),
                        ..HttpMatch::default()
                    }],
                    backend_refs: vec![backend_ref("default", "payments", 8080)],
                    ..HttpRule::default()
                },
            ],
            ..HttpRoute::default()
        }],
        grpc_routes: vec![GrpcRoute {
            name: "orders-grpc".to_string(),
            namespace: "default".to_string(),
            parent_refs: vec![],
            rules: vec![GrpcRule {
                name: String::new(),
                backend_refs: vec![backend_ref("default", "orders", 8080)],
                ..GrpcRule::default()
            }],
            ..GrpcRoute::default()
        }],
        stream_routes: vec![StreamRoute {
            name: "orders-tcp".to_string(),
            namespace: "default".to_string(),
            kind: "TCPRoute".to_string(),
            rules: vec![StreamRule {
                name: String::new(),
                matches: vec![StreamMatch {
                    port: 9000,
                    ..StreamMatch::default()
                }],
                backend_refs: vec![backend_ref("default", "orders", 8080)],
            }],
            ..StreamRoute::default()
        }],
        backends,
        ..Snapshot::default()
    }
}
