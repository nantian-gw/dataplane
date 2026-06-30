use super::*;

#[test]
fn selects_exact_path_before_prefix_match() {
    let snapshot = Snapshot {
        listeners: vec![listener_with_hostnames(
            "default/gw/http",
            &["api.example.com"],
            &["default/prefix-route", "default/exact-route"],
        )],
        http_routes: vec![
            HttpRoute {
                name: "prefix-route".to_string().into(),
                namespace: "default".to_string().into(),
                hostnames: vec!["api.example.com".to_string()],
                parent_refs: vec![],
                rules: vec![path_rule("/orders", "default", "orders-prefix", 8080)],
                labels: BTreeMap::new(),
                annotations: BTreeMap::new(),
            },
            HttpRoute {
                name: "exact-route".to_string().into(),
                namespace: "default".to_string().into(),
                hostnames: vec!["api.example.com".to_string()],
                parent_refs: vec![],
                rules: vec![HttpRule {
                    name: String::new(),
                    matches: vec![HttpMatch {
                        path: "/orders".to_string(),
                        path_type: "Exact".to_string(),
                        ..HttpMatch::default()
                    }],
                    filters: vec![],
                    backend_refs: vec![backend_ref("default", "orders-exact", 8080)],
                    timeouts: None,
                    retry: None,
                    session_persistence: None,
                }],
                labels: BTreeMap::new(),
                annotations: BTreeMap::new(),
            },
        ],
        backends: vec![
            backend_cluster("default", "orders-prefix", "10.0.0.10"),
            backend_cluster("default", "orders-exact", "10.0.0.11"),
        ],
        ..Snapshot::default()
    };

    let selected = snapshot
        .select_backend(&RequestMeta::new(
            Some("api.example.com".to_string()),
            "/orders",
            "GET",
            BTreeMap::new(),
        ))
        .expect("exact backend");

    assert_eq!(selected.route_name, "exact-route");
    assert_eq!(selected.backend.address, "10.0.0.11");
}

#[test]
fn http_and_grpc_routes_on_same_listener_are_selected_by_request_type() {
    let snapshot = Snapshot {
        listeners: vec![listener_with_hostnames(
            "default/gw/http",
            &["api.example.com"],
            &["default/http-route", "default/grpc-route"],
        )],
        http_routes: vec![HttpRoute {
            name: "http-route".to_string().into(),
            namespace: "default".to_string().into(),
            hostnames: vec!["api.example.com".to_string()],
            parent_refs: vec![],
            rules: vec![HttpRule {
                name: String::new(),
                matches: vec![HttpMatch {
                    path: "/helloworld.Greeter/SayHello".to_string(),
                    path_type: "Exact".to_string(),
                    ..HttpMatch::default()
                }],
                filters: vec![],
                backend_refs: vec![backend_ref("default", "http-backend", 8080)],
                timeouts: None,
                retry: None,
                session_persistence: None,
            }],
            labels: BTreeMap::new(),
            annotations: BTreeMap::new(),
        }],
        grpc_routes: vec![GrpcRoute {
            name: "grpc-route".to_string().into(),
            namespace: "default".to_string().into(),
            hostnames: vec!["api.example.com".to_string()],
            parent_refs: vec![],
            rules: vec![GrpcRule {
                name: String::new(),
                matches: vec![GrpcMatch {
                    service: "helloworld.Greeter".to_string(),
                    method: "SayHello".to_string(),
                    match_type: "Exact".to_string(),
                    ..GrpcMatch::default()
                }],
                filters: vec![],
                backend_refs: vec![backend_ref("default", "grpc-backend", 8080)],
                session_persistence: None,
            }],
            labels: BTreeMap::new(),
            annotations: BTreeMap::new(),
        }],
        backends: vec![
            backend_cluster("default", "http-backend", "10.0.0.21"),
            backend_cluster("default", "grpc-backend", "10.0.0.22"),
        ],
        ..Snapshot::default()
    };

    let http = snapshot
        .select_backend(&RequestMeta::new(
            Some("api.example.com".to_string()),
            "/helloworld.Greeter/SayHello",
            "POST",
            headers(&[("content-type", "application/json")]),
        ))
        .expect("http backend");
    assert_eq!(http.route_kind, RouteKind::Http);
    assert_eq!(http.route_name, "http-route");
    assert_eq!(http.backend.address, "10.0.0.21");

    let grpc = snapshot
        .select_backend(&RequestMeta::new(
            Some("api.example.com".to_string()),
            "/helloworld.Greeter/SayHello",
            "POST",
            headers(&[("content-type", "application/grpc+proto")]),
        ))
        .expect("grpc backend");
    assert_eq!(grpc.route_kind, RouteKind::Grpc);
    assert_eq!(grpc.route_name, "grpc-route");
    assert_eq!(grpc.backend.address, "10.0.0.22");
}
