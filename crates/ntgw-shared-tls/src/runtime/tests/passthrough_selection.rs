#[test]
fn passthrough_listener_selection_does_not_fall_back_to_less_specific_listener() {
    let shared = Snapshot::shared();
    shared.store(Arc::new(Snapshot {
        listeners: vec![
            tls_passthrough_listener(
                "default/gw/less-specific",
                vec!["*.example.com"],
                vec!["default/narrow-route"],
            ),
            tls_passthrough_listener(
                "default/gw/catch-all",
                Vec::new(),
                vec!["default/wide-route"],
            ),
        ],
        stream_routes: vec![
            ntgw_ir::StreamRoute {
                name: "narrow-route".to_string(),
                namespace: "default".to_string(),
                kind: "ROUTE_KIND_TLS".to_string(),
                rules: vec![ntgw_ir::StreamRule {
                    name: String::new(),
                    matches: vec![ntgw_ir::StreamMatch {
                        port: 443,
                        sni_hostname: "api.example.com".to_string(),
                        mode: ntgw_ir::TlsRouteMode::default(),
                    }],
                    backend_refs: vec![stream_backend_ref("narrow-backend")],
                }],
                ..ntgw_ir::StreamRoute::default()
            },
            ntgw_ir::StreamRoute {
                name: "wide-route".to_string(),
                namespace: "default".to_string(),
                kind: "ROUTE_KIND_TLS".to_string(),
                rules: vec![ntgw_ir::StreamRule {
                    name: String::new(),
                    matches: vec![ntgw_ir::StreamMatch {
                        port: 443,
                        sni_hostname: "*.com".to_string(),
                        mode: ntgw_ir::TlsRouteMode::default(),
                    }],
                    backend_refs: vec![stream_backend_ref("wide-backend")],
                }],
                ..ntgw_ir::StreamRoute::default()
            },
        ],
        backends: vec![
            stream_backend("narrow-backend"),
            stream_backend("wide-backend"),
        ],
        ..Snapshot::default()
    }));
    let mut s = (**shared.load()).clone();
    s.rebuild_runtime_indexes();
    shared.store(Arc::new(s));

    let listener_names = vec![
        "default/gw/less-specific".to_string(),
        "default/gw/catch-all".to_string(),
    ];

    assert_eq!(
        select_passthrough_listener(&shared, listener_names.as_slice(), Some("api.example.com"))
            .as_deref(),
        Some("default/gw/less-specific")
    );
    assert_eq!(
        select_passthrough_listener(&shared, listener_names.as_slice(), Some("other.example.com")),
        Some("default/gw/less-specific".to_string()),
        "selection must keep the best passthrough listener so the connection can be rejected instead of falling through"
    );
}

#[test]
fn passthrough_listener_selection_uses_listener_match_before_backend_resolution() {
    let shared = Snapshot::shared();
    shared.store(Arc::new(Snapshot {
        listeners: vec![tls_passthrough_listener(
            "default/gw/tls",
            vec!["example.com"],
            vec!["default/invalid-route"],
        )],
        stream_routes: vec![ntgw_ir::StreamRoute {
            name: "invalid-route".to_string(),
            namespace: "default".to_string(),
            kind: "ROUTE_KIND_TLS".to_string(),
            rules: vec![ntgw_ir::StreamRule {
                name: String::new(),
                matches: vec![ntgw_ir::StreamMatch {
                    port: 443,
                    sni_hostname: "example.com".to_string(),
                    mode: ntgw_ir::TlsRouteMode::default(),
                }],
                backend_refs: vec![ntgw_ir::BackendRef {
                    namespace: "default".to_string(),
                    name: "missing-backend".to_string(),
                    port: 443,
                    metadata: BTreeMap::from([(
                        "nantian.dev/backend-ref-valid".to_string(),
                        "false".to_string(),
                    )]),
                    ..ntgw_ir::BackendRef::default()
                }],
            }],
            ..ntgw_ir::StreamRoute::default()
        }],
        ..Snapshot::default()
    }));
    let mut s = (**shared.load()).clone();
    s.rebuild_runtime_indexes();
    shared.store(Arc::new(s));

    let listener_names = vec!["default/gw/tls".to_string()];

    assert_eq!(
        select_passthrough_listener(&shared, listener_names.as_slice(), Some("example.com")),
        Some("default/gw/tls".to_string()),
        "invalid or missing backend refs should reject on the passthrough surface instead of falling through to TLS termination"
    );
}

#[test]
fn passthrough_listener_selection_prefers_same_score_listener_with_route() {
    let shared = Snapshot::shared();
    shared.store(Arc::new(Snapshot {
        listeners: vec![
            tls_passthrough_listener(
                "default/gw/no-route",
                vec!["example.com"],
                vec!["default/other-route"],
            ),
            tls_passthrough_listener(
                "default/gw/with-route",
                vec!["example.com"],
                vec!["default/matching-route"],
            ),
        ],
        stream_routes: vec![
            ntgw_ir::StreamRoute {
                name: "other-route".to_string(),
                namespace: "default".to_string(),
                kind: "ROUTE_KIND_TLS".to_string(),
                rules: vec![ntgw_ir::StreamRule {
                    name: String::new(),
                    matches: vec![ntgw_ir::StreamMatch {
                        port: 443,
                        sni_hostname: "other.example.com".to_string(),
                        mode: ntgw_ir::TlsRouteMode::default(),
                    }],
                    backend_refs: vec![stream_backend_ref("other-backend")],
                }],
                ..ntgw_ir::StreamRoute::default()
            },
            ntgw_ir::StreamRoute {
                name: "matching-route".to_string(),
                namespace: "default".to_string(),
                kind: "ROUTE_KIND_TLS".to_string(),
                rules: vec![ntgw_ir::StreamRule {
                    name: String::new(),
                    matches: vec![ntgw_ir::StreamMatch {
                        port: 443,
                        sni_hostname: "example.com".to_string(),
                        mode: ntgw_ir::TlsRouteMode::default(),
                    }],
                    backend_refs: vec![stream_backend_ref("matching-backend")],
                }],
                ..ntgw_ir::StreamRoute::default()
            },
        ],
        backends: vec![
            stream_backend("other-backend"),
            stream_backend("matching-backend"),
        ],
        ..Snapshot::default()
    }));
    let mut s = (**shared.load()).clone();
    s.rebuild_runtime_indexes();
    shared.store(Arc::new(s));

    let listener_names = vec![
        "default/gw/no-route".to_string(),
        "default/gw/with-route".to_string(),
    ];

    assert_eq!(
        select_passthrough_listener(&shared, listener_names.as_slice(), Some("example.com")),
        Some("default/gw/with-route".to_string())
    );
}

#[test]
fn passthrough_listener_selection_treats_wildcards_as_suffix_matches() {
    let shared = Snapshot::shared();
    shared.store(Arc::new(Snapshot {
        listeners: vec![
            tls_passthrough_listener(
                "default/gw/wildcard-com",
                vec!["*.com"],
                vec!["default/wildcard-route"],
            ),
            tls_passthrough_listener(
                "default/gw/fallback",
                Vec::new(),
                vec!["default/fallback-route"],
            ),
        ],
        stream_routes: vec![
            ntgw_ir::StreamRoute {
                name: "wildcard-route".to_string(),
                namespace: "default".to_string(),
                kind: "ROUTE_KIND_TLS".to_string(),
                rules: vec![ntgw_ir::StreamRule {
                    name: String::new(),
                    matches: vec![ntgw_ir::StreamMatch {
                        port: 443,
                        sni_hostname: "*.com".to_string(),
                        mode: ntgw_ir::TlsRouteMode::default(),
                    }],
                    backend_refs: vec![stream_backend_ref("wildcard-backend")],
                }],
                ..ntgw_ir::StreamRoute::default()
            },
            ntgw_ir::StreamRoute {
                name: "fallback-route".to_string(),
                namespace: "default".to_string(),
                kind: "ROUTE_KIND_TLS".to_string(),
                rules: vec![ntgw_ir::StreamRule {
                    name: String::new(),
                    matches: vec![ntgw_ir::StreamMatch {
                        port: 443,
                        sni_hostname: "*.com".to_string(),
                        mode: ntgw_ir::TlsRouteMode::default(),
                    }],
                    backend_refs: vec![stream_backend_ref("fallback-backend")],
                }],
                ..ntgw_ir::StreamRoute::default()
            },
        ],
        backends: vec![
            stream_backend("wildcard-backend"),
            stream_backend("fallback-backend"),
        ],
        ..Snapshot::default()
    }));
    let mut s = (**shared.load()).clone();
    s.rebuild_runtime_indexes();
    shared.store(Arc::new(s));

    let listener_names = vec![
        "default/gw/wildcard-com".to_string(),
        "default/gw/fallback".to_string(),
    ];

    assert_eq!(
        select_passthrough_listener(&shared, listener_names.as_slice(), Some("non.matching.com")),
        Some("default/gw/wildcard-com".to_string()),
        "Gateway API wildcard hostnames are suffix matches, so *.com must outrank an empty hostname listener for nested names"
    );
}

#[test]
fn terminate_listener_selection_rejects_unmatched_sni() {
    let shared = Snapshot::shared();
    shared.store(Arc::new(Snapshot {
        listeners: vec![Listener {
            name: "default/gw/https".to_string(),
            address: "0.0.0.0".to_string(),
            addresses: vec!["0.0.0.0".to_string()],
            port: 443,
            protocol: "LISTENER_PROTOCOL_HTTPS".to_string(),
            hostnames: vec!["*.org".to_string()],
            attached_routes: vec!["default/https-route".to_string()],
            tls: Some(TlsConfig {
                enabled: true,
                passthrough: false,
                secret_refs: Vec::new(),
                sni_hosts: Vec::new(),
                min_version: String::new(),
                max_version: String::new(),
                frontend_validation: None,
            }),
            ..Listener::default()
        }],
        http_routes: vec![ntgw_ir::HttpRoute {
            name: "https-route".to_string(),
            namespace: "default".to_string(),
            hostnames: vec!["*.org".to_string()],
            ..ntgw_ir::HttpRoute::default()
        }],
        ..Snapshot::default()
    }));
    let mut s = (**shared.load()).clone();
    s.rebuild_runtime_indexes();
    shared.store(Arc::new(s));

    let listener_names = vec!["default/gw/https".to_string()];

    assert_eq!(
        select_terminate_listener(&shared, listener_names.as_slice(), Some("api.org")),
        Some("default/gw/https".to_string())
    );
    assert_eq!(
        select_terminate_listener(&shared, listener_names.as_slice(), Some("example.com")),
        None,
        "TLS termination should not present a fallback certificate when no listener hostname matches the SNI"
    );
}

fn tls_passthrough_listener(
    name: &str,
    hostnames: Vec<&str>,
    attached_routes: Vec<&str>,
) -> Listener {
    Listener {
        name: name.to_string(),
        address: "0.0.0.0".to_string(),
        addresses: vec!["0.0.0.0".to_string()],
        port: 443,
        protocol: "LISTENER_PROTOCOL_TLS_PASSTHROUGH".to_string(),
        hostnames: hostnames.into_iter().map(str::to_string).collect(),
        attached_routes: attached_routes.into_iter().map(str::to_string).collect(),
        tls: Some(TlsConfig {
            enabled: true,
            passthrough: true,
            secret_refs: Vec::new(),
            sni_hosts: Vec::new(),
            min_version: String::new(),
            max_version: String::new(),
            frontend_validation: None,
        }),
        ..Listener::default()
    }
}

fn stream_backend_ref(name: &str) -> ntgw_ir::BackendRef {
    ntgw_ir::BackendRef {
        namespace: "default".to_string(),
        name: name.to_string(),
        port: 443,
        ..ntgw_ir::BackendRef::default()
    }
}

fn stream_backend(name: &str) -> ntgw_ir::BackendCluster {
    ntgw_ir::BackendCluster {
        ai_service: None,
        token_policy: None,
        name: format!("{name}:443"),
        namespace: "default".to_string(),
        protocol: "TCP".to_string(),
        endpoints: vec![ntgw_ir::BackendEndpoint {
            address: "10.0.0.10".to_string(),
            port: 443,
            healthy: true,
        }],
        wasm_plugin: None,
        circuit_breaker: None,
        security_policy: None,
    }
}
