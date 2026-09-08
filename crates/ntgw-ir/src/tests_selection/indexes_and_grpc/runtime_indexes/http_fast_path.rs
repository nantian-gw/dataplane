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
fn http_fast_path_uses_hostname_index_to_visit_only_candidate_routes() {
    let mut snapshot = Snapshot {
        listeners: vec![fast_path_listener(&[
            "exact",
            "wildcard",
            "catch-all",
            "other",
            "needs-header",
        ])],
        http_routes: vec![
            fast_path_route("exact", &["api.example.com"], 8080),
            fast_path_route("wildcard", &["*.example.com"], 8081),
            fast_path_route("catch-all", &[], 8082),
            fast_path_route("other", &["other.example.net"], 8083),
            fast_path_header_route("needs-header", &["*.example.com"], 8084),
        ],
        backends: vec![
            fast_path_backend("exact", 8080, "10.0.0.10"),
            fast_path_backend("wildcard", 8081, "10.0.0.11"),
            fast_path_backend("catch-all", 8082, "10.0.0.12"),
            fast_path_backend("other", 8083, "10.0.0.13"),
            fast_path_backend("needs-header", 8084, "10.0.0.14"),
        ],
        ..Snapshot::default()
    };
    snapshot.rebuild_runtime_indexes();

    assert_eq!(snapshot.http_fast_path.route_count(), 4);
    assert_eq!(
        snapshot
            .http_fast_path
            .candidate_route_indices(&snapshot, Some("api.example.com")),
        vec![0, 1, 2]
    );
    assert_eq!(
        snapshot
            .http_fast_path
            .candidate_route_indices(&snapshot, Some("www.example.com")),
        vec![1, 2]
    );
    assert_eq!(
        snapshot
            .http_fast_path
            .candidate_route_indices(&snapshot, Some("other.example.net")),
        vec![2, 3]
    );
    assert_eq!(
        snapshot.http_fast_path.candidate_route_indices(&snapshot, None),
        vec![2]
    );
}

#[test]
fn http_fast_path_selects_indexed_exact_wildcard_and_catch_all_routes() {
    let mut snapshot = Snapshot {
        listeners: vec![fast_path_listener(&["catch-all", "exact", "wildcard"])],
        http_routes: vec![
            fast_path_route("catch-all", &[], 8080),
            fast_path_route("exact", &["api.example.com"], 8081),
            fast_path_route("wildcard", &["*.example.com"], 8082),
        ],
        backends: vec![
            fast_path_backend("catch-all", 8080, "10.0.0.10"),
            fast_path_backend("exact", 8081, "10.0.0.11"),
            fast_path_backend("wildcard", 8082, "10.0.0.12"),
        ],
        ..Snapshot::default()
    };
    snapshot.rebuild_runtime_indexes();

    assert_eq!(
        fast_path_selected_backend_name(&snapshot, Some("api.example.com")),
        "default/exact:8081"
    );
    assert_eq!(
        fast_path_selected_backend_name(&snapshot, Some("www.example.com")),
        "default/wildcard:8082"
    );
    assert_eq!(
        fast_path_selected_backend_name(&snapshot, Some("unmatched.example.net")),
        "default/catch-all:8080"
    );
    assert_eq!(
        fast_path_selected_backend_name(&snapshot, None),
        "default/catch-all:8080"
    );
}

#[test]
fn http_fast_path_falls_back_to_full_plan_when_runtime_indexes_unavailable() {
    let mut snapshot = Snapshot {
        listeners: vec![fast_path_listener(&["catch-all", "exact", "wildcard"])],
        http_routes: vec![
            fast_path_route("catch-all", &[], 8080),
            fast_path_route("exact", &["api.example.com"], 8081),
            fast_path_route("wildcard", &["*.example.com"], 8082),
        ],
        backends: vec![
            fast_path_backend("catch-all", 8080, "10.0.0.10"),
            fast_path_backend("exact", 8081, "10.0.0.11"),
            fast_path_backend("wildcard", 8082, "10.0.0.12"),
        ],
        ..Snapshot::default()
    };
    snapshot.rebuild_runtime_indexes();
    snapshot.runtime_indexes_ready = false;

    assert_eq!(
        snapshot
            .http_fast_path
            .candidate_route_indices(&snapshot, Some("api.example.com")),
        vec![0, 1, 2]
    );
    assert_eq!(
        fast_path_selected_backend_name(&snapshot, Some("api.example.com")),
        "default/exact:8081"
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

fn fast_path_listener(route_names: &[&str]) -> Listener {
    Listener {
        name: "default/gw/http".to_string(),
        port: 80,
        protocol: "HTTP".to_string(),
        attached_routes: route_names
            .iter()
            .map(|route_name| format!("default/{route_name}"))
            .collect(),
        ..Listener::default()
    }
}

fn fast_path_route(name: &str, hostnames: &[&str], port: u32) -> HttpRoute {
    HttpRoute {
        name: name.to_string(),
        namespace: "default".to_string(),
        hostnames: hostnames.iter().map(|hostname| (*hostname).to_string()).collect(),
        rules: vec![HttpRule {
            name: String::new(),
            matches: vec![HttpMatch {
                path: "/".to_string(),
                path_type: "PathPrefix".to_string(),
                method: "GET".to_string(),
                ..HttpMatch::default()
            }],
            backend_refs: vec![backend_ref("default", name, port)],
            ..HttpRule::default()
        }],
        ..HttpRoute::default()
    }
}

fn fast_path_header_route(name: &str, hostnames: &[&str], port: u32) -> HttpRoute {
    let mut route = fast_path_route(name, hostnames, port);
    route.rules[0].matches[0].headers = vec![HeaderMatch {
        name: "x-env".to_string(),
        value: "prod".to_string(),
        match_type: "Exact".to_string(),
        ..HeaderMatch::default()
    }];
    route
}

fn fast_path_backend(name: &str, port: u32, address: &str) -> BackendCluster {
    BackendCluster {
        name: format!("{name}:{port}"),
        namespace: "default".to_string(),
        protocol: "HTTP".to_string(),
        endpoints: vec![BackendEndpoint {
            address: address.to_string(),
            port,
            healthy: true,
        }],
        ..BackendCluster::default()
    }
}

fn fast_path_selected_backend_name(snapshot: &Snapshot, host: Option<&str>) -> String {
    snapshot
        .select_http_fast_path(crate::HttpFastPathRequest {
            host,
            port: 80,
            path: "/items",
            method: "GET",
            is_grpc: false,
        })
        .expect("fast path selected backend")
        .backend_name
        .to_string()
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
    assert_eq!(fast.route_name.as_ref(), generic.route_name.as_str());
    assert_eq!(fast.route_namespace.as_ref(), generic.route_namespace.as_str());
    assert_eq!(fast.rule_index, generic.rule_index);
    assert_eq!(fast.listener_name.as_ref(), generic.listener_name.as_str());
    assert_eq!(
        fast.listener_protocol.as_ref(),
        generic.listener_protocol.as_str()
    );
    assert_eq!(fast.backend_name.as_ref(), generic.backend_name.as_str());
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
