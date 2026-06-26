#[test]
fn selects_all_request_mirrors_and_strips_mirror_filters() {
    let snapshot = Snapshot {
        backends: vec![
            BackendCluster {
                name: "shadow-a:8081".to_string(),
                namespace: "observability".to_string(),
                protocol: "HTTP".to_string(),
                endpoints: vec![BackendEndpoint {
                    address: "10.0.0.60".to_string(),
                    port: 8081,
                    healthy: true,
                }],
                wasm_plugin: None,
                ai_service: None,
                token_policy: None,
            
                circuit_breaker: None,},
            BackendCluster {
                name: "shadow-b:8082".to_string(),
                namespace: "observability".to_string(),
                protocol: "HTTP".to_string(),
                endpoints: vec![BackendEndpoint {
                    address: "10.0.0.61".to_string(),
                    port: 8082,
                    healthy: true,
                }],
                wasm_plugin: None,
                ai_service: None,
                token_policy: None,
            
                circuit_breaker: None,},
        ],
        ..Snapshot::default()
    };

    let mirrors = snapshot.select_request_mirrors(&crate::RequestMirrorContext {
        route_kind: RouteKind::Http,
        route_name: "route".to_string(),
        route_namespace: "default".to_string(),
        rule_index: None,
        filters: vec![
            Filter {
                filter_type: "RequestHeaderModifier".to_string(),
                ..Filter::default()
            },
            Filter {
                filter_type: "RequestMirror".to_string(),
                request_mirror: Some(crate::RequestMirrorFilter {
                    backend_ref: BackendRef {
                        namespace: "observability".to_string(),
                        name: "shadow-a".to_string(),
                        port: 8081,
                        ..BackendRef::default()
                    },
                    ..crate::RequestMirrorFilter::default()
                }),
                ..Filter::default()
            },
            Filter {
                filter_type: "RequestMirror".to_string(),
                request_mirror: Some(crate::RequestMirrorFilter {
                    backend_ref: BackendRef {
                        namespace: "observability".to_string(),
                        name: "shadow-b".to_string(),
                        port: 8082,
                        ..BackendRef::default()
                    },
                    ..crate::RequestMirrorFilter::default()
                }),
                ..Filter::default()
            },
        ],
        matched_http_path: None,
        timeouts: None,
        backend_tls: None,
    });

    assert_eq!(mirrors.len(), 2);
    assert_eq!(mirrors[0].backend_name, "observability/shadow-a:8081");
    assert_eq!(mirrors[1].backend_name, "observability/shadow-b:8082");
    assert_eq!(mirrors[0].filters.len(), 1);
    assert_eq!(mirrors[0].filters[0].filter_type, "RequestHeaderModifier");
    assert_eq!(mirrors[1].filters.len(), 1);
    assert_eq!(mirrors[1].filters[0].filter_type, "RequestHeaderModifier");
}

#[test]
fn request_mirror_visitor_stops_after_first_selected_mirror() {
    let snapshot = Snapshot {
        backends: vec![
            BackendCluster {
                name: "shadow-a:8081".to_string(),
                namespace: "observability".to_string(),
                protocol: "HTTP".to_string(),
                endpoints: vec![BackendEndpoint {
                    address: "10.0.0.60".to_string(),
                    port: 8081,
                    healthy: true,
                }],
                wasm_plugin: None,
                ai_service: None,
                token_policy: None,
            
                circuit_breaker: None,},
            BackendCluster {
                name: "shadow-b:8082".to_string(),
                namespace: "observability".to_string(),
                protocol: "HTTP".to_string(),
                endpoints: vec![BackendEndpoint {
                    address: "10.0.0.61".to_string(),
                    port: 8082,
                    healthy: true,
                }],
                wasm_plugin: None,
                ai_service: None,
                token_policy: None,
            
                circuit_breaker: None,},
        ],
        ..Snapshot::default()
    };

    let context = crate::RequestMirrorContext {
        route_kind: RouteKind::Http,
        route_name: "route".to_string(),
        route_namespace: "default".to_string(),
        rule_index: None,
        filters: vec![
            Filter {
                filter_type: "RequestHeaderModifier".to_string(),
                ..Filter::default()
            },
            Filter {
                filter_type: "RequestMirror".to_string(),
                request_mirror: Some(crate::RequestMirrorFilter {
                    backend_ref: BackendRef {
                        namespace: "observability".to_string(),
                        name: "shadow-a".to_string(),
                        port: 8081,
                        ..BackendRef::default()
                    },
                    ..crate::RequestMirrorFilter::default()
                }),
                ..Filter::default()
            },
            Filter {
                filter_type: "RequestMirror".to_string(),
                request_mirror: Some(crate::RequestMirrorFilter {
                    backend_ref: BackendRef {
                        namespace: "observability".to_string(),
                        name: "shadow-b".to_string(),
                        port: 8082,
                        ..BackendRef::default()
                    },
                    ..crate::RequestMirrorFilter::default()
                }),
                ..Filter::default()
            },
        ],
        matched_http_path: None,
        timeouts: None,
        backend_tls: None,
    };

    let mut visited = Vec::new();
    snapshot.visit_request_mirrors(&context, |mirror| {
        visited.push(mirror.backend_name);
        false
    });

    assert_eq!(visited, vec!["observability/shadow-a:8081"]);
}
