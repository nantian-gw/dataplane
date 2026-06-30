#[test]
fn request_mirror_does_not_change_primary_backend_selection() {
    let snapshot = Snapshot {
        http_routes: vec![HttpRoute {
            name: "mirror".to_string(),
            namespace: "default".to_string(),
            hostnames: vec!["api.example.com".to_string()],
            parent_refs: vec![],
            rules: vec![HttpRule {
                name: String::new(),
                matches: vec![],
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
                                name: "shadow".to_string(),
                                port: 8081,
                                ..BackendRef::default()
                            },
                            ..crate::RequestMirrorFilter::default()
                        }),
                        ..Filter::default()
                    },
                ],
                backend_refs: vec![backend_ref("default", "users", 8080)],
                timeouts: None,
                retry: None,
                session_persistence: None,
            }],
            labels: BTreeMap::new(),
            annotations: BTreeMap::new(),
        }],
        backends: vec![
            BackendCluster {
                name: "users:8080".to_string(),
                namespace: "default".to_string(),
                protocol: "HTTP".to_string(),
                endpoints: vec![BackendEndpoint {
                    address: "10.0.0.70".to_string(),
                    port: 8080,
                    healthy: true,
                }],
                wasm_plugin: None,
                ai_service: None,
                token_policy: None,
            
                circuit_breaker: None,},
            BackendCluster {
                name: "shadow:8081".to_string(),
                namespace: "observability".to_string(),
                protocol: "HTTP".to_string(),
                endpoints: vec![BackendEndpoint {
                    address: "10.0.0.71".to_string(),
                    port: 8081,
                    healthy: true,
                }],
                wasm_plugin: None,
                ai_service: None,
                token_policy: None,
            
                circuit_breaker: None,},
        ],
        ..Snapshot::default()
    };

    let selected = snapshot
        .select_backend(&RequestMeta::new(
            Some("api.example.com".to_string()),
            "/",
            "GET",
            BTreeMap::new(),
        ))
        .expect("primary backend");

    assert_eq!(selected.route_kind, RouteKind::Http);
    assert_eq!(selected.backend_name, "default/users:8080");
    assert_eq!(selected.backend.address, "10.0.0.70");
    assert_eq!(selected.filters.len(), 2);
    assert_eq!(selected.filters[0].filter_type, "RequestHeaderModifier");
    assert_eq!(selected.filters[1].filter_type, "RequestMirror");

    let mirror = snapshot
        .select_request_mirror(&crate::RequestMirrorContext { route_policy: None,
            route_kind: selected.route_kind,
            route_name: selected.route_name.clone(),
            route_namespace: selected.route_namespace.clone(),
            rule_index: None,
            filters: selected.filters.clone(),
            matched_http_path: selected.matched_http_path.clone(),
            timeouts: selected.timeouts.clone(),
            backend_tls: selected.backend_tls.clone(),
        })
        .expect("mirror backend");
    assert_eq!(mirror.backend_name, "observability/shadow:8081");
    assert_eq!(mirror.backend.address, "10.0.0.71");
    assert_eq!(mirror.filters.len(), 1);
    assert_eq!(mirror.filters[0].filter_type, "RequestHeaderModifier");
}
