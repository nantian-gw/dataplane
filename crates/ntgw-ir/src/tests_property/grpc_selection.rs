proptest! {
    #[test]
    fn grpc_route_selection_matches_generated_path_and_metadata(
        service_segments in prop_vec(grpc_identifier_strategy(), 1..4),
        method in grpc_identifier_strategy(),
        header_name in header_name_strategy(),
        header_value in header_value_strategy(),
    ) {
        let service = service_segments.join(".");
        let snapshot = Snapshot {
            grpc_routes: vec![GrpcRoute {
                name: "generated-grpc".to_string().into(),
                namespace: "default".to_string().into(),
                hostnames: vec!["grpc.example.com".to_string()],
                parent_refs: vec![],
                rules: vec![GrpcRule {
                    name: String::new(),
                    matches: vec![GrpcMatch {
                        service: service.clone(),
                        method: method.clone(),
                        match_type: "Exact".to_string(),
                        headers: vec![HeaderMatch {
                            name: header_name.clone(),
                            value: header_value.clone(),
                            match_type: "Exact".to_string(),
                            ..HeaderMatch::default()
                        }],
                        ..GrpcMatch::default()
                    }],
                    filters: vec![],
                    backend_refs: vec![backend_ref("default", "generated", 8080)],
                    session_persistence: None,
                }],
                labels: std::collections::BTreeMap::new(),
                annotations: std::collections::BTreeMap::new(),
            }],
            backends: vec![BackendCluster {
                name: "generated:8080".to_string().into(),
                namespace: "default".to_string().into(),
                protocol: "HTTP".to_string().into(),
                endpoints: vec![BackendEndpoint {
                    address: "10.0.0.10".to_string(),
                    port: 8080,
                    healthy: true,
                }],
                wasm_plugin: None,
                ai_service: None,
                token_policy: None,
            
                circuit_breaker: None,}],
            ..Snapshot::default()
        };
        let mut headers = std::collections::BTreeMap::from([(
            "content-type".to_string(),
            vec!["application/grpc+proto".to_string()],
        )]);
        headers.insert(header_name.to_ascii_lowercase(), vec![header_value.clone()]);
        let path = format!("/{service}/{method}");
        let request = RequestMeta::new(
            Some("grpc.example.com".to_string()),
            path.as_str(),
            "post",
            headers,
        );

        let parsed = crate::parse_grpc_path(path.as_str()).expect("grpc path should parse");
        prop_assert_eq!(parsed.service, service);
        prop_assert_eq!(parsed.method, method);
        prop_assert!(crate::is_grpc_request(&request));

        let selected = snapshot
            .select_backend(&request)
            .expect("generated grpc route should match");
        prop_assert_eq!(selected.route_kind, RouteKind::Grpc);
        prop_assert_eq!(selected.route_name, "generated-grpc");
        prop_assert_eq!(selected.backend_name, "default/generated:8080");
    }
}
