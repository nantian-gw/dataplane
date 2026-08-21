#[test]
fn rebuild_runtime_indexes_precompiles_regex_matchers() {
    let mut snapshot = Snapshot {
        http_routes: vec![HttpRoute {
            name: "regex-http".to_string(),
            namespace: "default".to_string(),
            hostnames: vec!["example.com".to_string()],
            parent_refs: vec![],
            rules: vec![HttpRule {
                name: String::new(),
                matches: vec![HttpMatch {
                    path: "^/users/[0-9]+$".to_string(),
                    path_type: "RegularExpression".to_string(),
                    headers: vec![HeaderMatch {
                        name: "X-Tenant".to_string(),
                        value: "team-[a-z]+".to_string(),
                        match_type: "RegularExpression".to_string(),
                        ..HeaderMatch::default()
                    }],
                    query_params: vec![QueryMatch {
                        name: "Debug".to_string(),
                        value: "true|false".to_string(),
                        match_type: "RegularExpression".to_string(),
                        ..QueryMatch::default()
                    }],
                    ..HttpMatch::default()
                }],
                filters: vec![],
                backend_refs: vec![backend_ref("default", "users", 8080)],
                timeouts: None,
                retry: None,
                session_persistence: None,
            }],
            labels: BTreeMap::new(),
            annotations: BTreeMap::new(),
        }],
        security_policy: None,
        grpc_routes: vec![GrpcRoute {
            name: "regex-grpc".to_string(),
            namespace: "default".to_string(),
            hostnames: vec!["grpc.example.com".to_string()],
            parent_refs: vec![],
            rules: vec![GrpcRule {
                name: String::new(),
                matches: vec![GrpcMatch {
                    service: "helloworld\\..+".to_string(),
                    method: "Say(H|G).*".to_string(),
                    match_type: "RegularExpression".to_string(),
                    headers: vec![HeaderMatch {
                        name: "X-Region".to_string(),
                        value: "us-(east|west)-[0-9]+".to_string(),
                        match_type: "RegularExpression".to_string(),
                        ..HeaderMatch::default()
                    }],
                    ..GrpcMatch::default()
                }],
                filters: vec![],
                backend_refs: vec![backend_ref("default", "greeter", 9090)],
                session_persistence: None,
            }],
            labels: BTreeMap::new(),
            annotations: BTreeMap::new(),
        }],
        ..Snapshot::default()
    };

    snapshot.rebuild_runtime_indexes();

    let http_match = &snapshot.http_routes[0].rules[0].matches[0];
    assert!(http_match.compiled_path_regex.is_some());
    assert_eq!(http_match.headers[0].name, "x-tenant");
    assert!(http_match.headers[0].compiled_regex.is_some());
    assert_eq!(http_match.query_params[0].name, "debug");
    assert!(http_match.query_params[0].compiled_regex.is_some());

    let grpc_match = &snapshot.grpc_routes[0].rules[0].matches[0];
    assert!(grpc_match.compiled_service_regex.is_some());
    assert!(grpc_match.compiled_method_regex.is_some());
    assert_eq!(grpc_match.headers[0].name, "x-region");
    assert!(grpc_match.headers[0].compiled_regex.is_some());
}
