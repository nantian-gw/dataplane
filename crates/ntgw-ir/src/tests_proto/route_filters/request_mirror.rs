#[test]
fn decodes_request_mirror_filter_from_proto() {
    let snapshot = Snapshot::from(proto::ConfigSnapshot {
        http_routes: vec![proto::HttpRoute {
            name: "route".to_string(),
            namespace: "default".to_string(),
            rules: vec![proto::HttpRule {
                name: String::new(),
                filters: vec![proto::Filter {
                    r#type: "RequestMirror".to_string(),
                    config: Some(Struct {
                        fields: BTreeMap::from([
                            (
                                "backendRef".to_string(),
                                struct_proto_value(BTreeMap::from([
                                    ("namespace".to_string(), string_proto_value("observability")),
                                    ("name".to_string(), string_proto_value("shadow")),
                                    ("port".to_string(), number_proto_value(8081.0)),
                                ])),
                            ),
                            (
                                "fraction".to_string(),
                                struct_proto_value(BTreeMap::from([
                                    ("numerator".to_string(), number_proto_value(1.0)),
                                    ("denominator".to_string(), number_proto_value(2.0)),
                                ])),
                            ),
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
    let mirror = filter.request_mirror.as_ref().expect("request mirror");

    assert_eq!(filter.filter_type, "RequestMirror");
    assert_eq!(mirror.backend_ref.namespace, "observability");
    assert_eq!(mirror.backend_ref.name, "shadow");
    assert_eq!(mirror.backend_ref.port, 8081);
    assert_eq!(mirror.fraction.as_ref().expect("fraction").numerator, 1);
    assert_eq!(mirror.fraction.as_ref().expect("fraction").denominator, 2);
}
