proptest! {
    #[test]
    fn proto_snapshot_deserialization_preserves_generated_http_fields(
        listener_name in resource_name_strategy(),
        route_name in resource_name_strategy(),
        backend_name in resource_name_strategy(),
        hostname in hostname_strategy(),
        filter_value in header_value_strategy(),
        backend_weight in 1u32..32,
        backend_port in 1u32..65535,
    ) {
        let snapshot = Snapshot::from(proto::ConfigSnapshot {
            listeners: vec![proto::Listener {
                name: listener_name.clone(),
                address: "127.0.0.1".to_string(),
                addresses: vec!["127.0.0.1".to_string()],
                port: 8080,
                protocol: proto::ListenerProtocol::ListenerHttp as i32,
                hostnames: vec![hostname.clone()],
                attached_routes: vec![format!("default/{route_name}")],
                ..proto::Listener::default()
            }],
            http_routes: vec![proto::HttpRoute {
                name: route_name.clone(),
                namespace: "default".to_string(),
                hostnames: vec![hostname.clone()],
                rules: vec![proto::HttpRule {
                    name: String::new(),
                    filters: vec![proto::Filter {
                        r#type: "RequestHeaderModifier".to_string(),
                        config: Some(Struct {
                            fields: std::collections::BTreeMap::from([(
                                "set".to_string(),
                                list_proto_value(vec![struct_proto_value(std::collections::BTreeMap::from([
                                    ("name".to_string(), string_proto_value("x-generated")),
                                    ("value".to_string(), string_proto_value(filter_value.as_str())),
                                ]))]),
                            )]),
                        }),
                    }],
                    backend_refs: vec![proto::BackendRef {
                        namespace: "default".to_string(),
                        name: backend_name.clone(),
                        port: backend_port,
                        weight: backend_weight,
                        ..proto::BackendRef::default()
                    }],
                    ..proto::HttpRule::default()
                }],
                ..proto::HttpRoute::default()
            }],
            backends: vec![proto::BackendCluster {
                ai_service: None,
                token_policy: None,
                name: format!("{backend_name}:{backend_port}"),
                namespace: "default".to_string(),
                protocol: "HTTP".to_string(),
                endpoints: vec![proto::BackendEndpoint {
                    address: "10.0.0.20".to_string(),
                    port: backend_port,
                    healthy: true,
                    zone: String::new(),
                }],
                wasm_plugin: None,
                ..proto::BackendCluster::default()
            }],
            ..proto::ConfigSnapshot::default()
        });

        prop_assert_eq!(snapshot.listeners.len(), 1);
        prop_assert_eq!(snapshot.listeners[0].name.as_str(), listener_name.as_str());
        prop_assert_eq!(snapshot.http_routes.len(), 1);
        prop_assert_eq!(snapshot.http_routes[0].name.as_str(), route_name.as_str());
        prop_assert_eq!(snapshot.http_routes[0].hostnames.as_slice(), [hostname.as_str()]);
        prop_assert_eq!(
            snapshot.http_routes[0].rules[0].backend_refs[0].name.as_str(),
            backend_name.as_str()
        );
        prop_assert_eq!(snapshot.http_routes[0].rules[0].backend_refs[0].weight, backend_weight);
        prop_assert_eq!(
            snapshot.http_routes[0].rules[0].filters[0]
                .header_modifier
                .as_ref()
                .expect("header modifier")
                .set[0]
                .value
                .as_str(),
            filter_value.as_str()
        );
        prop_assert_eq!(snapshot.backends.len(), 1);
        prop_assert_eq!(snapshot.backends[0].endpoints[0].port, backend_port);
    }
}
