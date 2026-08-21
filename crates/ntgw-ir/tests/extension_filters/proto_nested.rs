use super::*;

#[test]
fn decodes_extension_ref_direct_response_from_proto() {
    let snapshot = Snapshot::from(proto::ConfigSnapshot {
        id: "snap".to_string(),
        generated_at: None,
        listeners: vec![],
        http_routes: vec![proto::HttpRoute {
            route_policy: None,
            security_policy: None,
            name: "orders".to_string(),
            namespace: "default".to_string(),
            hostnames: vec!["example.com".to_string()],
            parent_refs: vec![],
            rules: vec![proto::HttpRule {
                name: String::new(),
                matches: vec![proto::HttpMatch {
                    path: "/".to_string(),
                    path_type: "PathPrefix".to_string(),
                    method: String::new(),
                    headers: vec![],
                    query_params: vec![],
                }],
                filters: vec![proto::Filter {
                    r#type: "ExtensionRef".to_string(),
                    config: Some(Struct {
                        fields: BTreeMap::from([
                            ("resolved".to_string(), bool_value(true)),
                            ("extensionType".to_string(), string_value("DirectResponse")),
                            (
                                "directResponse".to_string(),
                                struct_value(Struct {
                                    fields: BTreeMap::from([
                                        ("statusCode".to_string(), number_value(503.0)),
                                        ("body".to_string(), string_value("maintenance")),
                                    ]),
                                }),
                            ),
                        ]),
                    }),
                }],
                backend_refs: vec![],
                timeouts: None,
                retry: None,
                session_persistence: None,
            }],
            labels: HashMap::new(),
            annotations: HashMap::new(),
        }],
        grpc_routes: vec![],
        stream_routes: vec![],
        backends: vec![],
        secrets: vec![],
        extensions: None,
        required_features: vec![],
        compatibility_profile: String::new(),
        traceparent: String::new(),
    });

    let filter = &snapshot.http_routes[0].rules[0].filters[0];
    let extension = filter.extension_ref.as_ref().expect("extension ref");
    assert!(extension.resolved);
    assert_eq!(extension.extension_type, "DirectResponse");
    assert_eq!(
        extension
            .direct_response
            .as_ref()
            .expect("direct response")
            .status_code,
        503
    );
}
