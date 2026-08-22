#[test]
fn http_fast_path_marks_simple_http_route_eligible() {
    let mut snapshot = Snapshot {
        listeners: vec![Listener {
            name: "default/gw/http".to_string(),
            port: 80,
            protocol: "HTTP".to_string(),
            attached_routes: vec!["default/orders".to_string()],
            ..Listener::default()
        }],
        http_routes: vec![HttpRoute {
            name: "orders".to_string(),
            namespace: "default".to_string(),
            hostnames: vec!["example.com".to_string()],
            rules: vec![HttpRule {
                name: String::new(),
                matches: vec![HttpMatch {
                    path: "/".to_string(),
                    path_type: "PathPrefix".to_string(),
                    method: "GET".to_string(),
                    ..HttpMatch::default()
                }],
                backend_refs: vec![backend_ref("default", "orders", 8080)],
                ..HttpRule::default()
            }],
            ..HttpRoute::default()
        }],
        backends: vec![BackendCluster {
            name: "orders:8080".to_string(),
            namespace: "default".to_string(),
            protocol: "HTTP".to_string(),
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

    assert_eq!(snapshot.http_fast_path.route_count(), 1);
    assert_eq!(snapshot.http_fast_path.eligible_rule_count(), 1);
    assert_eq!(snapshot.http_fast_path.compiled_backend_ref_count(), 1);
}

#[test]
fn http_fast_path_visits_candidate_listeners_without_index_vectors() {
    let mut snapshot = Snapshot {
        listeners: vec![
            Listener {
                name: "default/gw/http-a".to_string(),
                port: 80,
                protocol: "HTTP".to_string(),
                ..Listener::default()
            },
            Listener {
                name: "default/gw/grpc".to_string(),
                port: 80,
                protocol: "GRPC".to_string(),
                ..Listener::default()
            },
            Listener {
                name: "default/gw/http-b".to_string(),
                port: 80,
                protocol: "HTTP".to_string(),
                ..Listener::default()
            },
            Listener {
                name: "default/gw/http-other-port".to_string(),
                port: 8080,
                protocol: "HTTP".to_string(),
                ..Listener::default()
            },
        ],
        ..Snapshot::default()
    };
    snapshot.rebuild_runtime_indexes();

    let mut port_80 = Vec::new();
    crate::http_fast_path::visit_fast_candidate_listeners(&snapshot, 80, |_, listener| {
        port_80.push(listener.name.as_str());
    });
    assert_eq!(port_80, vec!["default/gw/http-a", "default/gw/http-b"]);

    let mut all_http = Vec::new();
    crate::http_fast_path::visit_fast_candidate_listeners(&snapshot, 0, |_, listener| {
        all_http.push(listener.name.as_str());
    });
    assert_eq!(
        all_http,
        vec![
            "default/gw/http-a",
            "default/gw/http-b",
            "default/gw/http-other-port"
        ]
    );
}

#[test]
fn http_fast_path_rejects_routes_that_need_headers_or_filters() {
    let mut snapshot = Snapshot {
        http_routes: vec![
            HttpRoute {
                name: "header-route".to_string(),
                namespace: "default".to_string(),
                rules: vec![HttpRule {
                    name: String::new(),
                    matches: vec![HttpMatch {
                        headers: vec![HeaderMatch {
                            name: "x-env".to_string(),
                            value: "prod".to_string(),
                            match_type: "Exact".to_string(),
                            ..HeaderMatch::default()
                        }],
                        ..HttpMatch::default()
                    }],
                    backend_refs: vec![backend_ref("default", "orders", 8080)],
                    ..HttpRule::default()
                }],
                ..HttpRoute::default()
            },
            HttpRoute {
                name: "filter-route".to_string(),
                namespace: "default".to_string(),
                rules: vec![HttpRule {
                    name: String::new(),
                    filters: vec![Filter {
                        filter_type: "RequestHeaderModifier".to_string(),
                        ..Filter::default()
                    }],
                    backend_refs: vec![backend_ref("default", "orders", 8080)],
                    ..HttpRule::default()
                }],
                ..HttpRoute::default()
            },
        ],
        backends: vec![BackendCluster {
            name: "orders:8080".to_string(),
            namespace: "default".to_string(),
            protocol: "HTTP".to_string(),
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

    assert_eq!(snapshot.http_fast_path.route_count(), 0);
    assert_eq!(snapshot.http_fast_path.eligible_rule_count(), 0);
    assert_eq!(snapshot.http_fast_path.compiled_backend_ref_count(), 0);
}

#[test]
fn http_fast_path_rejects_unresolved_backend_refs_at_compile_time() {
    let mut snapshot = Snapshot {
        listeners: vec![Listener {
            name: "default/gw/http".to_string(),
            port: 80,
            protocol: "HTTP".to_string(),
            attached_routes: vec!["default/orders".to_string()],
            ..Listener::default()
        }],
        http_routes: vec![HttpRoute {
            name: "orders".to_string(),
            namespace: "default".to_string(),
            rules: vec![HttpRule {
                name: String::new(),
                backend_refs: vec![backend_ref("default", "missing", 8080)],
                ..HttpRule::default()
            }],
            ..HttpRoute::default()
        }],
        ..Snapshot::default()
    };

    snapshot.rebuild_runtime_indexes();

    assert_eq!(snapshot.http_fast_path.route_count(), 0);
    assert_eq!(snapshot.http_fast_path.eligible_rule_count(), 0);
    assert_eq!(snapshot.http_fast_path.compiled_backend_ref_count(), 0);
    assert!(snapshot
        .select_http_fast_path(crate::HttpFastPathRequest {
            host: None,
            port: 80,
            path: "/",
            method: "GET",
            is_grpc: false,
        })
        .is_none());
}

#[test]
fn http_fast_path_selects_same_simple_backend_as_generic_http_selection() {
    let mut snapshot = Snapshot {
        listeners: vec![Listener {
            name: "default/gw/http".to_string(),
            port: 80,
            protocol: "HTTP".to_string(),
            attached_routes: vec!["default/orders".to_string()],
            ..Listener::default()
        }],
        http_routes: vec![HttpRoute {
            name: "orders".to_string(),
            namespace: "default".to_string(),
            hostnames: vec!["example.com".to_string()],
            rules: vec![HttpRule {
                name: String::new(),
                matches: vec![HttpMatch {
                    path: "/".to_string(),
                    path_type: "PathPrefix".to_string(),
                    method: "GET".to_string(),
                    ..HttpMatch::default()
                }],
                backend_refs: vec![backend_ref("default", "orders", 8080)],
                ..HttpRule::default()
            }],
            ..HttpRoute::default()
        }],
        backends: vec![BackendCluster {
            name: "orders:8080".to_string(),
            namespace: "default".to_string(),
            protocol: "HTTP".to_string(),
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

    let generic = snapshot
        .select_backend(&RequestMeta::new(
            Some("example.com".to_string()),
            "/",
            "GET",
            BTreeMap::new(),
        ))
        .expect("generic selected backend");
    let fast = snapshot
        .select_http_fast_path(crate::HttpFastPathRequest {
            host: Some("example.com"),
            port: 80,
            path: "/",
            method: "GET",
            is_grpc: false,
        })
        .expect("fast selected backend");

    assert_eq!(fast.route_kind, RouteKind::Http);
    assert_eq!(fast.route_name.as_str(), generic.route_name.as_str());
    assert_eq!(fast.route_namespace.as_str(), generic.route_namespace.as_str());
    assert_eq!(fast.rule_index, generic.rule_index);
    assert_eq!(fast.listener_name.as_str(), generic.listener_name.as_str());
    assert_eq!(fast.listener_protocol.as_str(), generic.listener_protocol.as_str());
    assert_eq!(fast.backend_name.as_str(), generic.backend_name.as_str());
    assert_eq!(fast.backend.address, generic.backend.address);
    assert_eq!(fast.backend.port, generic.backend.port);
    assert_eq!(
        fast.runtime_ids.route,
        snapshot.http_route_runtime_id("default", "orders")
    );
    assert_eq!(
        fast.runtime_ids.backend,
        snapshot.backend_runtime_id("default/orders:8080")
    );
}

#[test]
fn http_fast_path_does_not_select_grpc_requests() {
    let mut snapshot = Snapshot {
        http_routes: vec![HttpRoute {
            name: "orders".to_string(),
            namespace: "default".to_string(),
            rules: vec![HttpRule {
                name: String::new(),
                backend_refs: vec![backend_ref("default", "orders", 8080)],
                ..HttpRule::default()
            }],
            ..HttpRoute::default()
        }],
        backends: vec![BackendCluster {
            name: "orders:8080".to_string(),
            namespace: "default".to_string(),
            protocol: "HTTP".to_string(),
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

    assert!(snapshot
        .select_http_fast_path(crate::HttpFastPathRequest {
            host: None,
            port: 0,
            path: "/",
            method: "POST",
            is_grpc: true,
        })
        .is_none());
}

#[test]
fn http_fast_path_falls_back_when_best_match_needs_listener_backend_tls() {
    let mut snapshot = Snapshot {
        listeners: vec![
            Listener {
                name: "default/gw/tls-backend".to_string(),
                port: 80,
                protocol: "HTTP".to_string(),
                attached_routes: vec!["default/orders".to_string()],
                backend_tls: Some(crate::BackendTlsConfig {
                    client_certificate_ref: "default/client-cert".to_string(),
                }),
                ..Listener::default()
            },
            Listener {
                name: "default/gw/plain".to_string(),
                port: 80,
                protocol: "HTTP".to_string(),
                attached_routes: vec!["default/fallback".to_string()],
                ..Listener::default()
            },
        ],
        http_routes: vec![
            HttpRoute {
                name: "orders".to_string(),
                namespace: "default".to_string(),
                rules: vec![HttpRule {
                    name: String::new(),
                    matches: vec![HttpMatch {
                        path: "/orders".to_string(),
                        path_type: "Exact".to_string(),
                        method: "GET".to_string(),
                        ..HttpMatch::default()
                    }],
                    backend_refs: vec![backend_ref("default", "orders", 8080)],
                    ..HttpRule::default()
                }],
                ..HttpRoute::default()
            },
            HttpRoute {
                name: "fallback".to_string(),
                namespace: "default".to_string(),
                rules: vec![HttpRule {
                    name: String::new(),
                    matches: vec![HttpMatch {
                        path: "/".to_string(),
                        path_type: "PathPrefix".to_string(),
                        method: "GET".to_string(),
                        ..HttpMatch::default()
                    }],
                    backend_refs: vec![backend_ref("default", "fallback", 8081)],
                    ..HttpRule::default()
                }],
                ..HttpRoute::default()
            },
        ],
        backends: vec![
            BackendCluster {
                name: "orders:8080".to_string(),
                namespace: "default".to_string(),
                protocol: "HTTP".to_string(),
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

                },
            BackendCluster {
                name: "fallback:8081".to_string(),
                namespace: "default".to_string(),
                protocol: "HTTP".to_string(),
                endpoints: vec![BackendEndpoint {
                    address: "10.0.0.11".to_string(),
                    port: 8081,
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
    snapshot.rebuild_runtime_indexes();

    let generic = snapshot
        .select_backend(&RequestMeta::with_port(
            None,
            80,
            "/orders",
            "GET",
            BTreeMap::new(),
        ))
        .expect("generic selected backend");
    assert_eq!(generic.backend_name, "default/orders:8080");
    assert!(generic.backend_tls.is_some());

    assert!(snapshot
        .select_http_fast_path(crate::HttpFastPathRequest {
            host: None,
            port: 80,
            path: "/orders",
            method: "GET",
            is_grpc: false,
        })
        .is_none());
}
