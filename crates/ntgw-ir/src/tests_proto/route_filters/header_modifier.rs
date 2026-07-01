#[test]
fn decodes_header_modifier_filter_from_proto() {
    let snapshot = Snapshot::from(proto::ConfigSnapshot {
        http_routes: vec![proto::HttpRoute {
            name: "route".to_string().into(),
            namespace: "default".to_string().into(),
            rules: vec![proto::HttpRule {
                name: String::new(),
                filters: vec![proto::Filter {
                    r#type: "RequestHeaderModifier".to_string(),
                    config: Some(Struct {
                        fields: BTreeMap::from([
                            (
                                "set".to_string(),
                                list_proto_value(vec![struct_proto_value(BTreeMap::from([
                                    ("name".to_string(), string_proto_value("x-user")),
                                    ("value".to_string(), string_proto_value("alice")),
                                ]))]),
                            ),
                            (
                                "remove".to_string(),
                                list_proto_value(vec![string_proto_value("x-remove")]),
                            ),
                        ]),
                    }),
                }],
                ..Default::default()
            }],
            ..Default::default()
        }],
        ..Default::default()
    });

    let filter = &snapshot.http_routes[0].rules[0].filters[0];
    let modifier = filter.header_modifier.as_ref().expect("header modifier");

    assert_eq!(filter.filter_type, "RequestHeaderModifier");
    assert_eq!(modifier.set[0].name, "x-user");
    assert_eq!(modifier.set[0].value, "alice");
    assert_eq!(modifier.remove, vec!["x-remove"]);
}
