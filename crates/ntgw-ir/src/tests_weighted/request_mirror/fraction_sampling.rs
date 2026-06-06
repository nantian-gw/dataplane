#[test]
fn request_mirror_fraction_sampling_uses_fraction_window() {
    let snapshot = Snapshot {
        backends: vec![BackendCluster {
            name: "shadow:8081".to_string(),
            namespace: "observability".to_string(),
            protocol: "HTTP".to_string(),
            endpoints: vec![BackendEndpoint {
                address: "10.0.0.61".to_string(),
                port: 8081,
                healthy: true,
            }],
            wasm_plugin: None,
                ai_service: None,
                token_policy: None,
        }],
        ..Snapshot::default()
    };

    let filters = vec![Filter {
        filter_type: "RequestMirror".to_string(),
        request_mirror: Some(crate::RequestMirrorFilter {
            backend_ref: BackendRef {
                namespace: "observability".to_string(),
                name: "shadow".to_string(),
                port: 8081,
                ..BackendRef::default()
            },
            fraction: Some(crate::Fraction {
                numerator: 1,
                denominator: 2,
            }),
            ..crate::RequestMirrorFilter::default()
        }),
        ..Filter::default()
    }];

    assert!(snapshot
        .select_request_mirror(&crate::RequestMirrorContext {
            route_kind: RouteKind::Http,
            route_name: "route".to_string(),
            route_namespace: "default".to_string(),
            rule_index: None,
            filters: filters.clone(),
            matched_http_path: None,
            timeouts: None,
            backend_tls: None,
        })
        .is_some());
    assert!(snapshot
        .select_request_mirror(&crate::RequestMirrorContext {
            route_kind: RouteKind::Http,
            route_name: "route".to_string(),
            route_namespace: "default".to_string(),
            rule_index: None,
            filters: filters.clone(),
            matched_http_path: None,
            timeouts: None,
            backend_tls: None,
        })
        .is_none());
    assert!(snapshot
        .select_request_mirror(&crate::RequestMirrorContext {
            route_kind: RouteKind::Http,
            route_name: "route".to_string(),
            route_namespace: "default".to_string(),
            rule_index: None,
            filters,
            matched_http_path: None,
            timeouts: None,
            backend_tls: None,
        })
        .is_some());
}
