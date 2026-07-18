#[test]
fn decodes_http_route_timeouts_from_proto() {
    let snapshot = Snapshot::from(proto::ConfigSnapshot {
        http_routes: vec![proto::HttpRoute {
            name: "route".to_string(),
            namespace: "default".to_string(),
            rules: vec![proto::HttpRule {
                name: String::new(),
                timeouts: Some(proto::HttpRouteTimeouts {
                    request: Some(prost_types::Duration {
                        seconds: 12,
                        nanos: 0,
                    }),
                    backend_request: Some(prost_types::Duration {
                        seconds: 3,
                        nanos: 0,
                    }),
                }),
                ..Default::default()
            }],
            ..Default::default()
        }],
        ..Default::default()
    });

    let timeouts = snapshot.http_routes[0].rules[0]
        .timeouts
        .as_ref()
        .expect("route timeouts");

    assert_eq!(timeouts.request, Some(std::time::Duration::from_secs(12)));
    assert_eq!(
        timeouts.backend_request,
        Some(std::time::Duration::from_secs(3))
    );
}

#[test]
fn from_proto_retains_http_grpc_and_stream_route_labels() {
    let snapshot = Snapshot::from(proto::ConfigSnapshot {
        http_routes: vec![proto::HttpRoute {
            name: "http-route".to_string(),
            namespace: "default".to_string(),
            labels: std::collections::HashMap::from([("team".to_string(), "edge".to_string())]),
            ..Default::default()
        }],
        grpc_routes: vec![proto::GrpcRoute {
            name: "grpc-route".to_string(),
            namespace: "default".to_string(),
            labels: std::collections::HashMap::from([("team".to_string(), "api".to_string())]),
            ..Default::default()
        }],
        stream_routes: vec![proto::StreamRoute {
            name: "stream-route".to_string(),
            namespace: "default".to_string(),
            labels: std::collections::HashMap::from([("team".to_string(), "tcp".to_string())]),
            ..Default::default()
        }],
        ..Default::default()
    });

    assert_eq!(
        snapshot.http_routes[0].labels.get("team"),
        Some(&"edge".to_string())
    );
    assert_eq!(
        snapshot.grpc_routes[0].labels.get("team"),
        Some(&"api".to_string())
    );
    assert_eq!(
        snapshot.stream_routes[0].labels.get("team"),
        Some(&"tcp".to_string())
    );
}

#[test]
fn selects_http_route_without_backend_for_redirect_rule() {
    let snapshot = Snapshot {
        http_routes: vec![HttpRoute {
            name: "redirect".to_string(),
            namespace: "default".to_string(),
            hostnames: vec!["example.com".to_string()],
            parent_refs: vec![],
            rules: vec![HttpRule {
                name: String::new(),
                matches: vec![HttpMatch {
                    path: "/legacy".to_string(),
                    path_type: "PathPrefix".to_string(),
                    ..HttpMatch::default()
                }],
                filters: vec![Filter {
                    filter_type: "RequestRedirect".to_string(),
                    request_redirect: Some(crate::RequestRedirectFilter {
                        scheme: "https".to_string(),
                        ..crate::RequestRedirectFilter::default()
                    }),
                    ..Filter::default()
                }],
                backend_refs: vec![],
                timeouts: None,
                retry: None,
                session_persistence: None,
            }],
            labels: BTreeMap::new(),
            annotations: BTreeMap::new(),
        }],
        ..Snapshot::default()
    };

    let route = snapshot
        .select_http_route(&RequestMeta::new(
            Some("example.com".to_string()),
            "/legacy/docs",
            "GET",
            BTreeMap::new(),
        ))
        .expect("route");

    assert_eq!(route.route_name, "redirect");
    assert!(route.backend.is_none());
    assert_eq!(route.matched_http_path.path, "/legacy");
    assert_eq!(
        route.filters[0]
            .request_redirect
            .as_ref()
            .expect("request redirect")
            .scheme,
        "https"
    );
}

#[test]
fn select_http_route_preserves_rule_then_backend_filter_order() {
    let snapshot = Snapshot {
        http_routes: vec![HttpRoute {
            name: "route".to_string(),
            namespace: "default".to_string(),
            hostnames: vec!["example.com".to_string()],
            parent_refs: vec![],
            rules: vec![HttpRule {
                name: String::new(),
                matches: vec![HttpMatch {
                    path: "/".to_string(),
                    path_type: "PathPrefix".to_string(),
                    ..HttpMatch::default()
                }],
                filters: vec![Filter {
                    filter_type: "RequestHeaderModifier".to_string(),
                    header_modifier: Some(crate::HeaderModifier {
                        set: vec![crate::HeaderOperation {
                            name: "x-rule".to_string(),
                            value: "1".to_string(),
                        }],
                        ..crate::HeaderModifier::default()
                    }),
                    ..Filter::default()
                }],
                backend_refs: vec![BackendRef {
                    namespace: "default".to_string(),
                    name: "echo".to_string(),
                    port: 8080,
                    filters: vec![Filter {
                        filter_type: "ResponseHeaderModifier".to_string(),
                        header_modifier: Some(crate::HeaderModifier {
                            set: vec![crate::HeaderOperation {
                                name: "x-backend".to_string(),
                                value: "1".to_string(),
                            }],
                            ..crate::HeaderModifier::default()
                        }),
                        ..Filter::default()
                    }],
                    ..BackendRef::default()
                }],
                ..HttpRule::default()
            }],
            labels: BTreeMap::new(),
            annotations: BTreeMap::new(),
        }],
        backends: vec![BackendCluster {
            name: "echo:8080".to_string().into(),
            namespace: "default".to_string().into(),
            protocol: "HTTP".to_string().into(),
            endpoints: vec![BackendEndpoint {
                address: "127.0.0.1".to_string(),
                port: 8080,
                healthy: true,
            }],
            wasm_plugin: None,
                ai_service: None,
                token_policy: None,
        
                circuit_breaker: None,}],
        ..Snapshot::default()
    };

    let route = snapshot
        .select_http_route(&RequestMeta::new(
            Some("example.com".to_string()),
            "/",
            "GET",
            BTreeMap::new(),
        ))
        .expect("route");

    assert_eq!(route.filters.len(), 2);
    assert_eq!(route.filters[0].filter_type, "RequestHeaderModifier");
    assert_eq!(route.filters[1].filter_type, "ResponseHeaderModifier");
}
