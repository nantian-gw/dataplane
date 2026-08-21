#[test]
fn selects_request_mirror_backend_and_strips_mirror_filter() {
    let snapshot = Snapshot {
        backends: vec![BackendCluster {
            name: "shadow:8081".to_string().into(),
            namespace: "observability".to_string().into(),
            protocol: "HTTP".to_string().into(),
            endpoints: vec![BackendEndpoint {
                address: "10.0.0.60".to_string(),
                port: 8081,
                healthy: true,
            }],
            wasm_plugin: None,
                ai_service: None,
                token_policy: None,
        
                circuit_breaker: None,}],
        security_policy: None,
        ..Snapshot::default()
    };

    let selected = snapshot
        .select_request_mirror(&crate::RequestMirrorContext { route_policy: None,
            route_kind: RouteKind::Http,
            route_name: "route".to_string(),
            route_namespace: "default".to_string(),
            rule_index: None,
            filters: vec![
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
                Filter {
                    filter_type: "RequestHeaderModifier".to_string(),
                    ..Filter::default()
                },
            ],
            matched_http_path: Some(crate::MatchedHttpPath {
                path: "/api".to_string(),
                path_type: "PathPrefix".to_string(),
            }),
            timeouts: Some(crate::RouteTimeouts {
                request: Some(std::time::Duration::from_secs(12)),
                backend_request: Some(std::time::Duration::from_secs(3)),
                connect: None,
                next_upstream: None,
            }),
            backend_tls: None,
        })
        .expect("mirror backend");

    assert_eq!(selected.backend.address, "10.0.0.60");
    assert_eq!(selected.filters.len(), 1);
    assert_eq!(selected.filters[0].filter_type, "RequestHeaderModifier");
    assert_eq!(
        selected
            .matched_http_path
            .as_ref()
            .expect("matched path")
            .path,
        "/api"
    );
    assert_eq!(
        selected
            .timeouts
            .as_ref()
            .expect("timeouts")
            .backend_request,
        Some(std::time::Duration::from_secs(3))
    );
}

