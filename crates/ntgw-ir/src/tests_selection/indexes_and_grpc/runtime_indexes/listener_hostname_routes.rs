#[test]
fn runtime_indexes_precompute_listener_and_hostname_route_candidates() {
    let mut snapshot = Snapshot {
        listeners: vec![
            Listener {
                name: "default/gw/http-80".to_string().into(),
                port: 80,
                protocol: "HTTP".to_string().into(),
                hostnames: vec!["api.example.com".to_string()],
                attached_routes: vec!["default/exact".to_string(), "default/wild".to_string()],
                ..Listener::default()
            },
            Listener {
                name: "default/gw/grpc-80".to_string().into(),
                port: 80,
                protocol: "GRPC".to_string().into(),
                hostnames: vec!["grpc.example.com".to_string()],
                attached_routes: vec!["default/grpc".to_string()],
                ..Listener::default()
            },
        ],
        http_routes: vec![
            HttpRoute {
                name: "exact".to_string().into(),
                namespace: "default".to_string().into(),
                hostnames: vec!["api.example.com".to_string()],
                ..HttpRoute::default()
            },
            HttpRoute {
                name: "wild".to_string().into(),
                namespace: "default".to_string().into(),
                hostnames: vec!["*.example.com".to_string()],
                ..HttpRoute::default()
            },
            HttpRoute {
                name: "catch-all".to_string().into(),
                namespace: "default".to_string().into(),
                hostnames: vec![],
                ..HttpRoute::default()
            },
        ],
        grpc_routes: vec![
            GrpcRoute {
                name: "grpc".to_string().into(),
                namespace: "default".to_string().into(),
                hostnames: vec!["grpc.example.com".to_string()],
                ..GrpcRoute::default()
            },
            GrpcRoute {
                name: "grpc-wild".to_string().into(),
                namespace: "default".to_string().into(),
                hostnames: vec!["*.example.com".to_string()],
                ..GrpcRoute::default()
            },
        ],
        ..Snapshot::default()
    };

    snapshot.rebuild_runtime_indexes();

    assert_eq!(snapshot.http_listener_indices, vec![0]);
    assert_eq!(snapshot.grpc_listener_indices, vec![0, 1]);
    assert_eq!(snapshot.http_listener_port_index.get(&80), Some(&vec![0]));
    assert_eq!(
        snapshot.grpc_listener_port_index.get(&80),
        Some(&vec![0, 1])
    );
    assert_eq!(
        snapshot
            .route_attachment_listener_index
            .listeners_for_route("default", "exact"),
        Some(&[0][..])
    );
    assert_eq!(
        snapshot
            .route_attachment_listener_index
            .listeners_for_route("default", "wild"),
        Some(&[0][..])
    );
    assert_eq!(
        snapshot
            .route_attachment_listener_index
            .listeners_for_route("default", "grpc"),
        Some(&[1][..])
    );
    assert!(
        snapshot
            .route_attachment_listener_index
            .contains_listener("default", "exact", 0)
    );
    assert!(
        !snapshot
            .route_attachment_listener_index
            .contains_listener("default", "exact", 1)
    );

    assert_eq!(snapshot.http_route_hostname_index.catch_all, vec![2]);
    assert_eq!(
        snapshot
            .http_route_hostname_index
            .exact
            .get("api.example.com"),
        Some(&vec![0])
    );
    assert_eq!(
        snapshot
            .http_route_hostname_index
            .wildcard_suffix
            .get("example.com"),
        Some(&vec![1])
    );
    assert_eq!(
        snapshot
            .http_route_hostname_index
            .candidate_indices(Some("api.example.com")),
        vec![0, 1, 2]
    );
    let mut visited_http_route_indices = Vec::new();
    snapshot
        .http_route_hostname_index
        .visit_candidate_indices(Some("api.example.com"), |index| {
            visited_http_route_indices.push(index);
            true
        });
    assert_eq!(visited_http_route_indices, vec![0, 1, 2]);
    assert_eq!(
        snapshot
            .grpc_route_hostname_index
            .exact
            .get("grpc.example.com"),
        Some(&vec![0])
    );
    assert_eq!(
        snapshot
            .grpc_route_hostname_index
            .wildcard_suffix
            .get("example.com"),
        Some(&vec![1])
    );
    assert_eq!(
        snapshot
            .grpc_route_hostname_index
            .candidate_indices(Some("grpc.example.com")),
        vec![0, 1]
    );
    let mut visited_grpc_route_indices = Vec::new();
    snapshot
        .grpc_route_hostname_index
        .visit_candidate_indices(Some("grpc.example.com"), |index| {
            visited_grpc_route_indices.push(index);
            true
        });
    assert_eq!(visited_grpc_route_indices, vec![0, 1]);
}

#[test]
fn hostname_route_index_visits_next_candidate_without_duplicate_sources() {
    let mut snapshot = Snapshot {
        http_routes: vec![
            HttpRoute {
                name: "exact-and-wild".to_string().into(),
                namespace: "default".to_string().into(),
                hostnames: vec!["api.example.com".to_string(), "*.example.com".to_string()],
                ..HttpRoute::default()
            },
            HttpRoute {
                name: "catch-all".to_string().into(),
                namespace: "default".to_string().into(),
                hostnames: vec![],
                ..HttpRoute::default()
            },
            HttpRoute {
                name: "wild".to_string().into(),
                namespace: "default".to_string().into(),
                hostnames: vec!["*.example.com".to_string()],
                ..HttpRoute::default()
            },
            HttpRoute {
                name: "exact".to_string().into(),
                namespace: "default".to_string().into(),
                hostnames: vec!["api.example.com".to_string()],
                ..HttpRoute::default()
            },
        ],
        ..Snapshot::default()
    };
    snapshot.rebuild_runtime_indexes();

    let index = &snapshot.http_route_hostname_index;
    let mut cursor = None;
    let mut candidates = Vec::new();
    while let Some(candidate) =
        index.next_candidate_index_after(Some("api.example.com"), cursor)
    {
        candidates.push(candidate);
        cursor = Some(candidate);
    }

    assert_eq!(candidates, vec![0, 1, 2, 3]);
    assert_eq!(
        index.candidate_indices(Some("api.example.com")),
        vec![0, 1, 2, 3]
    );
    assert_eq!(index.candidate_indices(None), vec![1]);
    assert_eq!(index.next_candidate_index_after(None, None), Some(1));
    assert_eq!(index.next_candidate_index_after(None, Some(1)), None);
}

#[test]
fn unbuilt_route_attachment_index_does_not_override_slow_path_route_attachment_lookup() {
    let stale_index = crate::RouteAttachmentListenerIndex::from_listeners(&[Listener {
        name: "default/gw/stale".to_string().into(),
        attached_routes: vec!["default/other".to_string()],
        ..Listener::default()
    }]);
    let snapshot = Snapshot {
        listeners: vec![Listener {
            name: "default/gw/http".to_string().into(),
            port: 80,
            protocol: "HTTP".to_string().into(),
            attached_routes: vec!["default/orders".to_string()],
            ..Listener::default()
        }],
        http_routes: vec![HttpRoute {
            name: "orders".to_string().into(),
            namespace: "default".to_string().into(),
            rules: vec![HttpRule {
                backend_refs: vec![backend_ref("default", "orders", 8080)],
                ..HttpRule::default()
            }],
            ..HttpRoute::default()
        }],
        backends: vec![BackendCluster {
            name: "orders:8080".to_string().into(),
            namespace: "default".to_string().into(),
            protocol: "HTTP".to_string().into(),
            endpoints: vec![BackendEndpoint {
                address: "10.0.0.10".to_string(),
                port: 8080,
                healthy: true,
            }],
            wasm_plugin: None,
                ai_service: None,
                token_policy: None,
        
                circuit_breaker: None,}],
        route_attachment_listener_index: stale_index,
        runtime_indexes_ready: false,
        ..Snapshot::default()
    };

    let selected = snapshot
        .select_backend(&RequestMeta {
            port: 80,
            method: "GET".to_string(),
            path: "/".to_string(),
            ..RequestMeta::default()
        })
        .expect("backend");

    assert_eq!(selected.route_name, "orders");
    assert_eq!(selected.listener_name, "default/gw/http");
}

#[test]
fn runtime_indexes_precompute_service_frontend_metadata_by_listener_name() {
    let mut snapshot = Snapshot {
        listeners: vec![
            Listener {
                name: "default/gw/service-http".to_string().into(),
                metadata: BTreeMap::from([
                    (
                        crate::mesh::FRONTEND_KIND_METADATA_KEY.to_string(),
                        crate::mesh::FRONTEND_KIND_SERVICE.to_string(),
                    ),
                    (
                        crate::mesh::FRONTEND_NAMESPACE_METADATA_KEY.to_string(),
                        "default".to_string(),
                    ),
                    (
                        crate::mesh::FRONTEND_NAME_METADATA_KEY.to_string(),
                        "orders".to_string(),
                    ),
                    (
                        crate::mesh::FRONTEND_PORT_METADATA_KEY.to_string(),
                        "8080".to_string(),
                    ),
                ]),
                ..Listener::default()
            },
            Listener {
                name: "default/gw/plain".to_string().into(),
                ..Listener::default()
            },
        ],
        ..Snapshot::default()
    };

    snapshot.rebuild_runtime_indexes();

    let frontend = snapshot
        .service_frontend_for_listener_name("default/gw/service-http")
        .expect("service frontend");
    assert_eq!(frontend.namespace, "default");
    assert_eq!(frontend.name, "orders");
    assert_eq!(frontend.port, 8080);
        assert!(
            snapshot
                .service_frontend_for_listener_name("default/gw/plain")
                .is_none()
    );
}

#[test]
fn unbuilt_service_frontend_index_does_not_override_slow_path_lookup() {
    let snapshot = Snapshot {
        listeners: vec![Listener {
            name: "default/gw/service-http".to_string().into(),
            metadata: BTreeMap::from([
                (
                    crate::mesh::FRONTEND_KIND_METADATA_KEY.to_string(),
                    crate::mesh::FRONTEND_KIND_SERVICE.to_string(),
                ),
                (
                    crate::mesh::FRONTEND_NAMESPACE_METADATA_KEY.to_string(),
                    "default".to_string(),
                ),
                (
                    crate::mesh::FRONTEND_NAME_METADATA_KEY.to_string(),
                    "orders".to_string(),
                ),
                (
                    crate::mesh::FRONTEND_PORT_METADATA_KEY.to_string(),
                    "8080".to_string(),
                ),
            ]),
            ..Listener::default()
        }],
        service_frontend_index: std::collections::HashMap::from([(
            "default/gw/service-http".to_string(),
            crate::mesh::ServiceFrontendRef {
                namespace: "stale".to_string().into(),
                name: "stale".to_string().into(),
                port: 19080,
            },
        )]),
        runtime_indexes_ready: false,
        ..Snapshot::default()
    };

    let frontend = snapshot
        .service_frontend_for_listener_name("default/gw/service-http")
        .expect("service frontend");

    assert_eq!(frontend.namespace, "default");
    assert_eq!(frontend.name, "orders");
    assert_eq!(frontend.port, 8080);
}

#[test]
fn runtime_indexes_precompute_service_frontend_attachment_lookup() {
    let mut snapshot = Snapshot {
        listeners: vec![
            Listener {
                name: "default/gw/service-http".to_string().into(),
                port: 8080,
                protocol: "HTTP".to_string().into(),
                metadata: service_frontend_metadata("default", "orders", 8080),
                ..Listener::default()
            },
            Listener {
                name: "default/gw/service-http-attached".to_string().into(),
                port: 8080,
                protocol: "HTTP".to_string().into(),
                attached_routes: vec!["default/orders-route".to_string()],
                metadata: service_frontend_metadata("default", "orders", 8080),
                ..Listener::default()
            },
        ],
        backends: vec![BackendCluster {
            name: "orders:8080".to_string().into(),
            namespace: "default".to_string().into(),
            protocol: "HTTP".to_string().into(),
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

    let selected = snapshot
        .select_backend(&RequestMeta::with_port(
            Some("orders.default.svc.cluster.local".to_string()),
            8080,
            "/",
            "GET",
            BTreeMap::new(),
        ))
        .expect("backend");

    assert_eq!(selected.listener_name, "default/gw/service-http");
    assert_eq!(selected.backend_name, "default/orders:8080");
    assert_eq!(selected.backend.address, "10.0.0.10");
}

#[test]
fn unbuilt_service_frontend_attachment_index_does_not_override_slow_path_lookup() {
    let snapshot = Snapshot {
        listeners: vec![Listener {
            name: "default/gw/service-http".to_string().into(),
            port: 8080,
            protocol: "HTTP".to_string().into(),
            metadata: service_frontend_metadata("default", "orders", 8080),
            ..Listener::default()
        }],
        backends: vec![BackendCluster {
            name: "orders:8080".to_string().into(),
            namespace: "default".to_string().into(),
            protocol: "HTTP".to_string().into(),
            endpoints: vec![BackendEndpoint {
                address: "10.0.0.10".to_string(),
                port: 8080,
                healthy: true,
            }],
            wasm_plugin: None,
                ai_service: None,
                token_policy: None,
        
                circuit_breaker: None,}],
        service_frontend_attachment_index: std::collections::HashMap::from([(
            "default".to_string(),
            std::collections::HashSet::from(["orders".to_string()]),
        )]),
        runtime_indexes_ready: false,
        ..Snapshot::default()
    };

    assert!(
        snapshot
            .select_backend(&RequestMeta::with_port(
                Some("orders.default.svc.cluster.local".to_string()),
                8080,
                "/",
                "GET",
                BTreeMap::new(),
            ))
            .is_none()
    );
}

#[test]
fn runtime_indexes_precompute_service_frontend_attachment_lookup_by_namespace() {
    let mut snapshot = Snapshot {
        listeners: vec![
            Listener {
                name: "default/gw/service-http".to_string().into(),
                metadata: service_frontend_metadata("default", "orders", 8080),
                attached_routes: vec!["default/orders-route".to_string()],
                ..Listener::default()
            },
            Listener {
                name: "default/gw/plain".to_string().into(),
                metadata: service_frontend_metadata("default", "payments", 8080),
                ..Listener::default()
            },
        ],
        ..Snapshot::default()
    };

    snapshot.rebuild_runtime_indexes();

    assert_eq!(
        snapshot.service_frontend_attachment_index.get("default"),
        Some(&std::collections::HashSet::from(["orders".to_string()]))
    );
    assert!(
        snapshot
            .service_frontend_attachment_index
            .get("default")
            .is_some_and(|names| names.contains("orders"))
    );
    assert!(
        snapshot
            .service_frontend_attachment_index
            .get("default")
            .is_none_or(|names| !names.contains("payments"))
    );
}

#[test]
fn runtime_indexes_precompute_service_frontend_port_lookup() {
    let mut snapshot = Snapshot {
        listeners: vec![
            Listener {
                name: "default/gw/service-http-first".to_string().into(),
                port: 8080,
                protocol: "HTTP".to_string().into(),
                metadata: service_frontend_metadata("default", "orders", 8080),
                ..Listener::default()
            },
            Listener {
                name: "default/gw/service-http-indexed".to_string().into(),
                port: 8080,
                protocol: "HTTP".to_string().into(),
                attached_routes: vec!["default/orders-route".to_string()],
                metadata: service_frontend_metadata("default", "orders", 8080),
                ..Listener::default()
            },
        ],
        backends: vec![BackendCluster {
            name: "orders:8080".to_string().into(),
            namespace: "default".to_string().into(),
            protocol: "HTTP".to_string().into(),
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
    assert_eq!(
        snapshot.service_frontend_listener_port_index.get(&8080),
        Some(&vec![0, 1])
    );
    snapshot
        .service_frontend_listener_port_index
        .insert(8080, vec![1, 0]);

    let selected = snapshot
        .select_backend(&RequestMeta::with_port(
            Some("orders.default.svc.cluster.local".to_string()),
            8080,
            "/",
            "GET",
            BTreeMap::new(),
        ))
        .expect("backend");

    assert_eq!(selected.listener_name, "default/gw/service-http-indexed");
    assert_eq!(selected.backend_name, "default/orders:8080");
}

#[test]
fn unbuilt_service_frontend_port_index_does_not_override_slow_path_lookup() {
    let snapshot = Snapshot {
        listeners: vec![
            Listener {
                name: "default/gw/service-http-first".to_string().into(),
                port: 8080,
                protocol: "HTTP".to_string().into(),
                metadata: service_frontend_metadata("default", "orders", 8080),
                ..Listener::default()
            },
            Listener {
                name: "default/gw/service-http-indexed".to_string().into(),
                port: 8080,
                protocol: "HTTP".to_string().into(),
                attached_routes: vec!["default/orders-route".to_string()],
                metadata: service_frontend_metadata("default", "orders", 8080),
                ..Listener::default()
            },
        ],
        backends: vec![BackendCluster {
            name: "orders:8080".to_string().into(),
            namespace: "default".to_string().into(),
            protocol: "HTTP".to_string().into(),
            endpoints: vec![BackendEndpoint {
                address: "10.0.0.10".to_string(),
                port: 8080,
                healthy: true,
            }],
            wasm_plugin: None,
                ai_service: None,
                token_policy: None,
        
                circuit_breaker: None,}],
        service_frontend_listener_port_index: std::collections::HashMap::from([(8080, vec![
            1, 0,
        ])]),
        runtime_indexes_ready: false,
        ..Snapshot::default()
    };

    let selected = snapshot
        .select_backend(&RequestMeta::with_port(
            Some("orders.default.svc.cluster.local".to_string()),
            8080,
            "/",
            "GET",
            BTreeMap::new(),
        ))
        .expect("backend");

    assert_eq!(selected.listener_name, "default/gw/service-http-first");
    assert_eq!(selected.backend_name, "default/orders:8080");
}

fn service_frontend_metadata(namespace: &str, name: &str, port: u32) -> BTreeMap<String, String> {
    BTreeMap::from([
        (
            crate::mesh::FRONTEND_KIND_METADATA_KEY.to_string(),
            crate::mesh::FRONTEND_KIND_SERVICE.to_string(),
        ),
        (
            crate::mesh::FRONTEND_NAMESPACE_METADATA_KEY.to_string(),
            namespace.to_string(),
        ),
        (
            crate::mesh::FRONTEND_NAME_METADATA_KEY.to_string(),
            name.to_string(),
        ),
        (
            crate::mesh::FRONTEND_PORT_METADATA_KEY.to_string(),
            port.to_string(),
        ),
    ])
}
