#[test]
fn decodes_backend_ref_weight_from_proto() {
    let snapshot = Snapshot::from(proto::ConfigSnapshot {
        http_routes: vec![proto::HttpRoute {
            name: "route".to_string(),
            namespace: "default".to_string(),
            rules: vec![proto::HttpRule {
                name: String::new(),
                backend_refs: vec![proto::BackendRef {
                    namespace: "default".to_string(),
                    name: "users".to_string(),
                    port: 8080,
                    weight: 7,
                    ..Default::default()
                }],
                ..Default::default()
            }],
            ..Default::default()
        }],
        ..Default::default()
    });

    assert_eq!(snapshot.http_routes[0].rules[0].backend_refs[0].weight, 7);
}

#[test]
fn decodes_backend_timeouts_from_proto() {
    let snapshot = Snapshot::from(proto::ConfigSnapshot {
        backends: vec![proto::BackendCluster {
            ai_service: None,
            token_policy: None,
            name: "orders:8080".to_string(),
            namespace: "default".to_string(),
            protocol: "HTTP".to_string(),
            connect_timeout: Some(prost_types::Duration {
                seconds: 5,
                nanos: 0,
            }),
            request_timeout: Some(prost_types::Duration {
                seconds: 30,
                nanos: 500_000_000,
            }),
            wasm_plugin: None,
            ..Default::default()
        }],
        ..Default::default()
    });

    let policy = snapshot
        .backend_policy("default/orders:8080")
        .expect("backend policy");
    assert_eq!(
        policy.connect_timeout,
        Some(std::time::Duration::from_secs(5))
    );
    assert_eq!(
        policy.request_timeout,
        Some(std::time::Duration::from_millis(30_500))
    );
}

#[test]
fn decodes_proto_snapshot_without_runtime_indexes_for_staged_apply() {
    let snapshot = Snapshot::from_proto_without_runtime_indexes(proto::ConfigSnapshot {
        backends: vec![proto::BackendCluster {
            ai_service: None,
            token_policy: None,
            name: "orders:8080".to_string(),
            namespace: "default".to_string(),
            protocol: "HTTP".to_string(),
            wasm_plugin: None,
            ..Default::default()
        }],
        ..Default::default()
    });

    assert!(!snapshot.runtime_indexes_ready);
    assert!(snapshot.backend_index.is_empty());

    let mut indexed = snapshot.clone();
    indexed.rebuild_runtime_indexes();
    assert!(indexed.runtime_indexes_ready);
    assert_eq!(indexed.backend_index.get("default/orders:8080"), Some(&0));
}

#[test]
fn decodes_zero_backend_request_timeout_as_unset() {
    let snapshot = Snapshot::from(proto::ConfigSnapshot {
        backends: vec![proto::BackendCluster {
            ai_service: None,
            token_policy: None,
            name: "orders:8080".to_string(),
            namespace: "default".to_string(),
            protocol: "HTTP".to_string(),
            connect_timeout: Some(prost_types::Duration {
                seconds: 5,
                nanos: 0,
            }),
            request_timeout: Some(prost_types::Duration {
                seconds: 0,
                nanos: 0,
            }),
            wasm_plugin: None,
            ..Default::default()
        }],
        ..Default::default()
    });

    let policy = snapshot
        .backend_policy("default/orders:8080")
        .expect("backend policy");
    assert_eq!(
        policy.connect_timeout,
        Some(std::time::Duration::from_secs(5))
    );
    assert_eq!(policy.request_timeout, None);
}

#[test]
fn decodes_backend_ref_filters_from_proto() {
    let snapshot = Snapshot::from(proto::ConfigSnapshot {
        http_routes: vec![proto::HttpRoute {
            name: "route".to_string(),
            namespace: "default".to_string(),
            rules: vec![proto::HttpRule {
                name: String::new(),
                backend_refs: vec![proto::BackendRef {
                    namespace: "default".to_string(),
                    name: "echo".to_string(),
                    port: 8080,
                    weight: 1,
                    filters: vec![proto::Filter {
                        r#type: "RequestHeaderModifier".to_string(),
                        config: Some(Struct {
                            fields: BTreeMap::from([(
                                "set".to_string(),
                                list_proto_value(vec![struct_proto_value(BTreeMap::from([
                                    ("name".to_string(), string_proto_value("X-Test")),
                                    ("value".to_string(), string_proto_value("value")),
                                ]))]),
                            )]),
                        }),
                    }],
                    ..Default::default()
                }],
                ..Default::default()
            }],
            ..Default::default()
        }],
        ..Default::default()
    });

    let backend_ref = &snapshot.http_routes[0].rules[0].backend_refs[0];
    assert_eq!(backend_ref.filters.len(), 1);
    assert_eq!(backend_ref.filters[0].filter_type, "RequestHeaderModifier");
    assert_eq!(
        backend_ref.filters[0]
            .header_modifier
            .as_ref()
            .expect("header modifier")
            .set[0]
            .name,
        "X-Test"
    );
}
