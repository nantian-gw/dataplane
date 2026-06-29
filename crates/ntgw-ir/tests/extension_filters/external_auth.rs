use super::*;

#[test]
fn decodes_external_auth_http_filter_from_proto() {
    let snapshot = Snapshot::from(proto::ConfigSnapshot {
        id: "snap".to_string(),
        generated_at: None,
        listeners: vec![],
        http_routes: vec![proto::HttpRoute { route_policy: None, 
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
                    r#type: "ExternalAuth".to_string(),
                    config: Some(Struct {
                        fields: BTreeMap::from([
                            ("protocol".to_string(), string_value("HTTP")),
                            (
                                "backendRef".to_string(),
                                struct_value(Struct {
                                    fields: BTreeMap::from([
                                        ("namespace".to_string(), string_value("security")),
                                        ("name".to_string(), string_value("auth-service")),
                                        ("port".to_string(), number_value(9000.0)),
                                    ]),
                                }),
                            ),
                            (
                                "http".to_string(),
                                struct_value(Struct {
                                    fields: BTreeMap::from([
                                        ("path".to_string(), string_value("/check")),
                                        (
                                            "allowedHeaders".to_string(),
                                            Value {
                                                kind: Some(Kind::ListValue(
                                                    prost_types::ListValue {
                                                        values: vec![
                                                            string_value("authorization"),
                                                            string_value("x-tenant"),
                                                        ],
                                                    },
                                                )),
                                            },
                                        ),
                                        (
                                            "allowedResponseHeaders".to_string(),
                                            Value {
                                                kind: Some(Kind::ListValue(
                                                    prost_types::ListValue {
                                                        values: vec![
                                                            string_value("x-user"),
                                                            string_value("x-scope"),
                                                        ],
                                                    },
                                                )),
                                            },
                                        ),
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
    });

    let filter = &snapshot.http_routes[0].rules[0].filters[0];
    let external_auth = filter.external_auth.as_ref().expect("external auth");
    assert_eq!(external_auth.protocol, "HTTP");
    assert_eq!(external_auth.backend_ref.namespace, "security");
    assert_eq!(external_auth.backend_ref.name, "auth-service");
    assert_eq!(external_auth.backend_ref.port, 9000);
    assert_eq!(external_auth.http.path, "/check");
    assert_eq!(
        external_auth.http.allowed_headers,
        ["authorization", "x-tenant"]
    );
    assert_eq!(
        external_auth.http.allowed_response_headers,
        ["x-user", "x-scope"]
    );
}

#[test]
fn decodes_external_auth_grpc_filter_from_proto() {
    let snapshot = Snapshot::from(proto::ConfigSnapshot {
        id: "snap".to_string(),
        generated_at: None,
        listeners: vec![],
        http_routes: vec![proto::HttpRoute { route_policy: None, 
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
                    r#type: "ExternalAuth".to_string(),
                    config: Some(Struct {
                        fields: BTreeMap::from([
                            ("protocol".to_string(), string_value("GRPC")),
                            (
                                "backendRef".to_string(),
                                struct_value(Struct {
                                    fields: BTreeMap::from([
                                        ("namespace".to_string(), string_value("security")),
                                        ("name".to_string(), string_value("grpc-auth")),
                                        ("port".to_string(), number_value(9000.0)),
                                    ]),
                                }),
                            ),
                            (
                                "grpc".to_string(),
                                struct_value(Struct {
                                    fields: BTreeMap::from([(
                                        "allowedHeaders".to_string(),
                                        Value {
                                            kind: Some(Kind::ListValue(prost_types::ListValue {
                                                values: vec![
                                                    string_value("authorization"),
                                                    string_value("x-tenant"),
                                                ],
                                            })),
                                        },
                                    )]),
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
    });

    let filter = &snapshot.http_routes[0].rules[0].filters[0];
    let external_auth = filter.external_auth.as_ref().expect("external auth");
    assert_eq!(external_auth.protocol, "GRPC");
    assert_eq!(external_auth.backend_ref.namespace, "security");
    assert_eq!(external_auth.backend_ref.name, "grpc-auth");
    assert_eq!(
        external_auth.grpc.allowed_headers,
        ["authorization", "x-tenant"]
    );
}

#[test]
fn decodes_external_auth_backend_with_tls_validation_from_proto() {
    let snapshot = Snapshot::from(proto::ConfigSnapshot {
        id: "snap".to_string(),
        generated_at: None,
        listeners: vec![],
        http_routes: vec![proto::HttpRoute { route_policy: None, 
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
                    r#type: "ExternalAuth".to_string(),
                    config: Some(Struct {
                        fields: BTreeMap::from([
                            ("protocol".to_string(), string_value("HTTP")),
                            (
                                "backendRef".to_string(),
                                struct_value(Struct {
                                    fields: BTreeMap::from([
                                        ("namespace".to_string(), string_value("default")),
                                        ("name".to_string(), string_value("auth")),
                                        ("port".to_string(), number_value(8443.0)),
                                    ]),
                                }),
                            ),
                            (
                                "http".to_string(),
                                struct_value(Struct {
                                    fields: BTreeMap::from([
                                        ("path".to_string(), string_value("/check")),
                                        (
                                            "allowedHeaders".to_string(),
                                            Value {
                                                kind: Some(Kind::ListValue(
                                                    prost_types::ListValue { values: vec![] },
                                                )),
                                            },
                                        ),
                                        (
                                            "allowedResponseHeaders".to_string(),
                                            Value {
                                                kind: Some(Kind::ListValue(
                                                    prost_types::ListValue { values: vec![] },
                                                )),
                                            },
                                        ),
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
        backends: vec![proto::BackendCluster {
            name: "auth:8443".to_string(),
            namespace: "default".to_string(),
            protocol: "HTTPS".to_string(),
            tls_validation: Some(proto::BackendTlsValidation {
                hostname: "auth.default.svc.cluster.local".to_string(),
                use_system_ca_certificates: true,
                ca_pems: vec![],
                min_version: String::new(),
                max_version: String::new(),
                subject_alt_names: vec![],
            }),
            wasm_plugin: None,
            ai_service: None,
            token_policy: None,
            ..Default::default()
        }],
        secrets: vec![],
        extensions: None,
        required_features: vec![],
        compatibility_profile: String::new(),
    });

    let filter = &snapshot.http_routes[0].rules[0].filters[0];
    let external_auth = filter.external_auth.as_ref().expect("external auth");
    assert_eq!(external_auth.protocol, "HTTP");
    assert_eq!(external_auth.backend_ref.name, "auth");
    assert_eq!(external_auth.backend_ref.port, 8443);

    let policy = snapshot
        .backend_policy("default/auth:8443")
        .expect("auth backend policy");
    let validation = policy
        .tls_validation
        .as_ref()
        .expect("auth backend tls validation");
    assert_eq!(validation.hostname, "auth.default.svc.cluster.local");
    assert!(validation.use_system_ca_certificates);
    assert!(validation.ca_pems.is_empty());
}
