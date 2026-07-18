#[test]
fn decodes_redirect_and_rewrite_filters_from_proto() {
    let snapshot = Snapshot::from(proto::ConfigSnapshot {
        http_routes: vec![proto::HttpRoute {
            name: "route".to_string(),
            namespace: "default".to_string(),
            rules: vec![proto::HttpRule {
                name: String::new(),
                filters: vec![
                    proto::Filter {
                        r#type: "RequestRedirect".to_string(),
                        config: Some(Struct {
                            fields: BTreeMap::from([
                                ("scheme".to_string(), string_proto_value("https")),
                                (
                                    "hostname".to_string(),
                                    string_proto_value("www.example.com"),
                                ),
                                ("port".to_string(), number_proto_value(8443.0)),
                                ("statusCode".to_string(), number_proto_value(301.0)),
                            ]),
                        }),
                    },
                    proto::Filter {
                        r#type: "URLRewrite".to_string(),
                        config: Some(Struct {
                            fields: BTreeMap::from([
                                (
                                    "hostname".to_string(),
                                    string_proto_value("backend.internal"),
                                ),
                                (
                                    "path".to_string(),
                                    struct_proto_value(BTreeMap::from([
                                        (
                                            "type".to_string(),
                                            string_proto_value("ReplacePrefixMatch"),
                                        ),
                                        (
                                            "replacePrefixMatch".to_string(),
                                            string_proto_value("/api"),
                                        ),
                                    ])),
                                ),
                            ]),
                        }),
                    },
                ],
                ..Default::default()
            }],
            ..Default::default()
        }],
        ..Default::default()
    });

    let filters = &snapshot.http_routes[0].rules[0].filters;
    let redirect = filters[0]
        .request_redirect
        .as_ref()
        .expect("request redirect");
    let rewrite = filters[1].url_rewrite.as_ref().expect("url rewrite");

    assert_eq!(redirect.scheme, "https");
    assert_eq!(redirect.hostname, "www.example.com");
    assert_eq!(redirect.port, 8443);
    assert_eq!(redirect.status_code, 301);
    assert_eq!(rewrite.hostname, "backend.internal");
    assert_eq!(
        rewrite
            .path
            .as_ref()
            .expect("path modifier")
            .replace_prefix_match,
        "/api"
    );
}
