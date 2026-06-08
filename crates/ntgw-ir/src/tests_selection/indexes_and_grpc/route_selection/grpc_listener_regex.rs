#[test]
fn selects_grpc_backend_on_http_listener() {
    let snapshot = Snapshot {
        listeners: vec![Listener {
            name: "default/gw/http".to_string(),
            address: "0.0.0.0".to_string(),
            addresses: vec!["0.0.0.0".to_string()],
            port: 80,
            protocol: "LISTENER_PROTOCOL_HTTP".to_string(),
            hostnames: vec!["grpc.example.com".to_string()],
            attached_routes: vec!["default/grpc-route".to_string()],
            tls: None,
            backend_tls: None,
            metadata: BTreeMap::new(),
        }],
        grpc_routes: vec![GrpcRoute {
            name: "grpc-route".to_string(),
            namespace: "default".to_string(),
            hostnames: vec!["grpc.example.com".to_string()],
            parent_refs: vec![],
            rules: vec![GrpcRule {
                name: String::new(),
                matches: vec![GrpcMatch {
                    service: "helloworld.Greeter".to_string(),
                    method: "SayHello".to_string(),
                    match_type: "Exact".to_string(),
                    headers: Vec::new(),
                    ..GrpcMatch::default()
                }],
                filters: vec![],
                backend_refs: vec![backend_ref("default", "greeter", 9090)],
                session_persistence: None,
            }],
            annotations: BTreeMap::new(),
        }],
        backends: vec![BackendCluster {
            name: "greeter:9090".to_string(),
            namespace: "default".to_string(),
            protocol: "GRPC".to_string(),
            endpoints: vec![BackendEndpoint {
                address: "10.0.0.30".to_string(),
                port: 9090,
                healthy: true,
            }],
            wasm_plugin: None,
                ai_service: None,
                token_policy: None,
        }],
        ..Snapshot::default()
    };

    let request = RequestMeta::new(
        Some("grpc.example.com".to_string()),
        "/helloworld.Greeter/SayHello",
        "POST",
        headers(&[("content-type", "application/grpc+proto")]),
    );

    let selected = snapshot.select_backend(&request).expect("backend");
    assert_eq!(selected.route_kind, RouteKind::Grpc);
    assert_eq!(selected.route_name, "grpc-route");
    assert_eq!(selected.backend.port, 9090);
}

#[test]
fn selects_grpc_backend_by_regex_service_and_method() {
    let snapshot = Snapshot {
        grpc_routes: vec![GrpcRoute {
            name: "grpc-regex-route".to_string(),
            namespace: "default".to_string(),
            hostnames: vec!["grpc.example.com".to_string()],
            parent_refs: vec![],
            rules: vec![GrpcRule {
                name: String::new(),
                matches: vec![GrpcMatch {
                    service: "helloworld\\..+".to_string(),
                    method: "Say(H|G).*".to_string(),
                    match_type: "RegularExpression".to_string(),
                    headers: Vec::new(),
                    ..GrpcMatch::default()
                }],
                filters: vec![],
                backend_refs: vec![backend_ref("default", "greeter", 9090)],
                session_persistence: None,
            }],
            annotations: BTreeMap::new(),
        }],
        backends: vec![BackendCluster {
            name: "greeter:9090".to_string(),
            namespace: "default".to_string(),
            protocol: "GRPC".to_string(),
            endpoints: vec![BackendEndpoint {
                address: "10.0.0.30".to_string(),
                port: 9090,
                healthy: true,
            }],
            wasm_plugin: None,
                ai_service: None,
                token_policy: None,
        }],
        ..Snapshot::default()
    };

    let request = RequestMeta::new(
        Some("grpc.example.com".to_string()),
        "/helloworld.Greeter/SayHello",
        "POST",
        headers(&[("content-type", "application/grpc+proto")]),
    );

    let selected = snapshot.select_backend(&request).expect("backend");
    assert_eq!(selected.route_kind, RouteKind::Grpc);
    assert_eq!(selected.route_name, "grpc-regex-route");
    assert_eq!(selected.backend.port, 9090);
}
