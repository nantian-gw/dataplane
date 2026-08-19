#[test]
fn decodes_cors_filter_from_proto() {
    let snapshot = Snapshot::from(proto::ConfigSnapshot {
        http_routes: vec![proto::HttpRoute {
            name: "route".to_string(),
            namespace: "default".to_string(),
            rules: vec![proto::HttpRule {
                name: String::new(),
                filters: vec![proto::Filter {
                    r#type: "CORS".to_string(),
                    config: Some(Struct {
                        fields: BTreeMap::from([
                            (
                                "allowOrigins".to_string(),
                                list_proto_value(vec![string_proto_value("https://app.example")]),
                            ),
                            (
                                "allowMethods".to_string(),
                                list_proto_value(vec![
                                    string_proto_value("GET"),
                                    string_proto_value("POST"),
                                ]),
                            ),
                            (
                                "allowHeaders".to_string(),
                                list_proto_value(vec![
                                    string_proto_value("authorization"),
                                    string_proto_value("content-type"),
                                ]),
                            ),
                            (
                                "exposeHeaders".to_string(),
                                list_proto_value(vec![string_proto_value("x-trace-id")]),
                            ),
                            ("allowCredentials".to_string(), bool_proto_value(true)),
                            ("maxAge".to_string(), number_proto_value(600.0)),
                        ]),
                    }),
                }],
                ..Default::default()
            }],
            ..Default::default()
        }],
        security_policy: None,
        ..Default::default()
    });

    let filter = &snapshot.http_routes[0].rules[0].filters[0];
    let cors = filter.cors.as_ref().expect("cors filter");

    assert_eq!(filter.filter_type, "CORS");
    assert_eq!(cors.allow_origins, vec!["https://app.example"]);
    assert_eq!(cors.allow_methods, vec!["GET", "POST"]);
    assert_eq!(cors.allow_headers, vec!["authorization", "content-type"]);
    assert_eq!(cors.expose_headers, vec!["x-trace-id"]);
    assert!(cors.allow_credentials);
    assert_eq!(cors.max_age, Some(600));
}
