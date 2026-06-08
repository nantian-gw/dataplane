use super::*;

#[test]
fn selects_http_route_with_direct_response_without_backend() {
    let snapshot = Snapshot {
        http_routes: vec![HttpRoute {
            name: "orders".to_string(),
            namespace: "default".to_string(),
            hostnames: vec!["example.com".to_string()],
            parent_refs: vec![],
            rules: vec![HttpRule {
                name: String::new(),
                matches: vec![HttpMatch {
                    path: "/".to_string(),
                    path_type: "PathPrefix".to_string(),
                    method: String::new(),
                    headers: vec![],
                    query_params: vec![],
                    ..HttpMatch::default()
                }],
                filters: vec![Filter {
                    filter_type: "ExtensionRef".to_string(),
                    extension_ref: Some(ExtensionFilter {
                        resolved: true,
                        extension_type: "DirectResponse".to_string(),
                        direct_response: Some(DirectResponseFilter {
                            status_code: 503,
                            body: "maintenance".to_string(),
                            content_type: "text/plain".to_string(),
                            headers: vec![],
                        }),
                        ..ExtensionFilter::default()
                    }),
                    ..Filter::default()
                }],
                backend_refs: vec![],
                ..HttpRule::default()
            }],
            ..HttpRoute::default()
        }],
        ..Snapshot::default()
    };

    let route = snapshot
        .select_http_route(&RequestMeta::with_port(
            Some("example.com".to_string()),
            80,
            "/",
            "GET",
            BTreeMap::new(),
        ))
        .expect("selected route");

    assert!(route.backend.is_none());
    assert_eq!(route.filters.len(), 1);
    assert!(
        route.filters[0]
            .extension_ref
            .as_ref()
            .and_then(|item| item.direct_response.as_ref())
            .is_some()
    );
}
