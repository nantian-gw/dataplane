#[test]
fn build_upstream_peer_with_cached_config_uses_policy_and_protocol_without_snapshot() {
    let selected = SelectedBackend { route_policy: None,
        route_kind: RouteKind::Http,
        route_name: "route".to_string(),
        route_namespace: "default".to_string(),
        rule_index: None,
        route_annotations: BTreeMap::new(),
        listener_name: "default/gw/http".to_string(),
        listener_protocol: "LISTENER_PROTOCOL_HTTP".to_string(),
        backend: BackendEndpoint {
            address: "127.0.0.1".to_string(),
            port: 8080,
            healthy: true,
        },
        backend_name: "default/orders:8080".to_string(),
        filters: Vec::new(),
        matched_http_path: None,
        timeouts: Some(RouteTimeouts {
            request: Some(std::time::Duration::from_secs(7)),
            backend_request: None,
            connect: None,
            next_upstream: None,
        }),
        retry: None,
        session_persistence: None,
        backend_tls: None,
    };

    let snapshot = Snapshot::default();
    let policy = BackendPolicy {
        connect_timeout: Some(std::time::Duration::from_secs(3)),
        request_timeout: Some(std::time::Duration::from_secs(11)),
        tls_validation: None,
        session_persistence: None,
        load_balancing: None,
        health_check: None,
        outlier_detection: None,
    };
    let config =
        selected_backend_config_with_overrides(&snapshot, &selected, Some("H2C"), Some(&policy))
            .expect("selected backend config");

    let peer = build_upstream_peer_with_cached_config(&selected, &config, None, &UpstreamTuningOptions::default()).expect("peer");

    assert_eq!(peer.options.alpn.get_min_http_version(), 2);
    assert_eq!(peer.options.alpn.get_max_http_version(), 2);
    assert_eq!(
        peer.options.connection_timeout,
        Some(std::time::Duration::from_secs(3))
    );
    assert_eq!(
        peer.options.read_timeout,
        Some(std::time::Duration::from_secs(7))
    );
}

#[test]
fn selected_backend_config_debug_exposes_precomputed_policy_fields_only() {
    let selected = SelectedBackend { route_policy: None,
        route_kind: RouteKind::Http,
        route_name: "route".to_string(),
        route_namespace: "default".to_string(),
        rule_index: None,
        route_annotations: BTreeMap::new(),
        listener_name: "default/gw/http".to_string(),
        listener_protocol: "LISTENER_PROTOCOL_HTTP".to_string(),
        backend: BackendEndpoint {
            address: "127.0.0.1".to_string(),
            port: 8080,
            healthy: true,
        },
        backend_name: "default/orders:8080".to_string(),
        filters: Vec::new(),
        matched_http_path: None,
        timeouts: None,
        retry: None,
        session_persistence: None,
        backend_tls: None,
    };
    let snapshot = Snapshot::default();
    let policy = BackendPolicy {
        connect_timeout: Some(std::time::Duration::from_secs(3)),
        request_timeout: Some(std::time::Duration::from_secs(11)),
        tls_validation: None,
        session_persistence: None,
        load_balancing: None,
        health_check: None,
        outlier_detection: None,
    };
    let config =
        selected_backend_config_with_overrides(&snapshot, &selected, Some("HTTP"), Some(&policy))
            .expect("selected backend config");

    let debug = format!("{config:?}");

    assert!(debug.contains("connect_timeout"));
    assert!(debug.contains("request_timeout"));
    assert!(
        !debug.contains("BackendPolicy"),
        "selected backend config should keep precomputed fields, not the full policy"
    );
}

#[test]
fn selected_backend_config_precomputes_peer_runtime_metadata() {
    let snapshot = Snapshot {
        backends: vec![BackendCluster {
            ai_service: None,
            token_policy: None,
            name: "orders:8443".to_string().into(),
            namespace: "default".to_string().into(),
            protocol: "GRPCS".to_string().into(),
            endpoints: vec![BackendEndpoint {
                address: "10.0.0.10".to_string(),
                port: 8443,
                healthy: true,
            }],
            wasm_plugin: None,
        
                circuit_breaker: None,
                security_policy: None,}],
        backend_policies: BTreeMap::from([(
            "default/orders:8443".to_string(),
            BackendPolicy {
                connect_timeout: Some(std::time::Duration::from_secs(3)),
                request_timeout: Some(std::time::Duration::from_secs(11)),
                tls_validation: Some(BackendTlsValidation {
                    hostname: "orders.internal".to_string(),
                    use_system_ca_certificates: true,
                    ca_pems: Vec::new(),
                    subject_alt_names: Vec::new(),
                    min_version: String::new(),
                    max_version: String::new(),
                }),
                session_persistence: None,
                load_balancing: None,
                health_check: None,
                outlier_detection: None,
            },
        )]),
        ..Snapshot::default()
    };
    let selected = SelectedBackend { route_policy: None,
        route_kind: RouteKind::Http,
        route_name: "route".to_string(),
        route_namespace: "default".to_string(),
        rule_index: None,
        route_annotations: BTreeMap::new(),
        listener_name: "default/gw/http".to_string(),
        listener_protocol: "LISTENER_PROTOCOL_HTTP".to_string(),
        backend: BackendEndpoint {
            address: "10.0.0.10".to_string(),
            port: 8443,
            healthy: true,
        },
        backend_name: "default/orders:8443".to_string(),
        filters: Vec::new(),
        matched_http_path: None,
        timeouts: Some(RouteTimeouts {
            request: None,
            backend_request: Some(std::time::Duration::from_secs(7)),
            connect: None,
            next_upstream: None,
        }),
        retry: None,
        session_persistence: None,
        backend_tls: None,
    };

    let config = selected_backend_config(&snapshot, &selected).expect("selected backend config");

    assert!(matches!(
        config.peer_address,
        UpstreamPeerAddress::Ip(std::net::IpAddr::V4(address)) if address.octets() == [10, 0, 0, 10]
    ));
    assert_eq!(config.peer_port, 8443);
    assert!(config.tls_enabled);
    assert!(config.use_http2);
    assert_eq!(config.sni, "orders.internal");
    assert_eq!(
        config.connect_timeout,
        Some(std::time::Duration::from_secs(3))
    );
    assert_eq!(
        config.request_timeout,
        Some(std::time::Duration::from_secs(7))
    );
}

#[test]
fn selected_backend_config_precomputes_resource_runtime_ids() {
    let mut snapshot = Snapshot {
        listeners: vec![ntgw_ir::Listener {
            name: "default/gw/http".to_string(),
            address: "0.0.0.0".to_string(),
            port: 80,
            protocol: "LISTENER_PROTOCOL_HTTP".to_string(),
            attached_routes: vec!["default/orders".to_string()],
            ..ntgw_ir::Listener::default()
        }],
        http_routes: vec![ntgw_ir::HttpRoute {
            name: "orders".to_string(),
            namespace: "default".to_string(),
            hostnames: vec!["orders.example.com".to_string()],
            rules: vec![ntgw_ir::HttpRule {
                name: String::new(),
                backend_refs: vec![ntgw_ir::BackendRef {
                    namespace: "default".to_string(),
                    name: "orders".to_string(),
                    port: 8080,
                    ..ntgw_ir::BackendRef::default()
                }],
                ..ntgw_ir::HttpRule::default()
            }],
            ..ntgw_ir::HttpRoute::default()
        }],
        backends: vec![BackendCluster {
            ai_service: None,
            token_policy: None,
            name: "orders:8080".to_string().into(),
            namespace: "default".to_string().into(),
            protocol: "HTTP".to_string().into(),
            endpoints: vec![BackendEndpoint {
                address: "10.0.0.10".to_string(),
                port: 8080,
                healthy: true,
            }],
            wasm_plugin: None,
        
                circuit_breaker: None,
                security_policy: None,}],
        ..Snapshot::default()
    };
    snapshot.rebuild_runtime_indexes();
    let selected = snapshot
        .select_backend(&ntgw_ir::RequestMeta::new(
            Some("orders.example.com".to_string()),
            "/",
            "GET",
            BTreeMap::new(),
        ))
        .expect("selected backend");

    let config = selected_backend_config(&snapshot, &selected).expect("selected backend config");

    assert_eq!(
        config.runtime_ids.listener,
        snapshot.listener_runtime_id("default/gw/http")
    );
    assert_eq!(
        config.runtime_ids.route,
        snapshot.http_route_runtime_id("default", "orders")
    );
    assert_eq!(
        config.runtime_ids.rule,
        snapshot.http_rule_runtime_id("default", "orders", 0)
    );
    assert_eq!(
        config.runtime_ids.backend,
        snapshot.backend_runtime_id("default/orders:8080")
    );
    assert_eq!(
        config.runtime_ids.endpoint,
        snapshot.endpoint_runtime_id("default/orders:8080", &selected.backend)
    );
}

#[test]
fn selected_backend_config_cache_reuses_config_for_snapshot_runtime_ids() {
    let mut snapshot = Snapshot {
        id: "snapshot-one".to_string(),
        listeners: vec![ntgw_ir::Listener {
            name: "default/gw/http".to_string(),
            address: "0.0.0.0".to_string(),
            port: 80,
            protocol: "LISTENER_PROTOCOL_HTTP".to_string(),
            attached_routes: vec!["default/orders".to_string()],
            ..ntgw_ir::Listener::default()
        }],
        http_routes: vec![ntgw_ir::HttpRoute {
            name: "orders".to_string(),
            namespace: "default".to_string(),
            hostnames: vec!["orders.example.com".to_string()],
            rules: vec![ntgw_ir::HttpRule {
                name: String::new(),
                backend_refs: vec![ntgw_ir::BackendRef {
                    namespace: "default".to_string(),
                    name: "orders".to_string(),
                    port: 8080,
                    ..ntgw_ir::BackendRef::default()
                }],
                ..ntgw_ir::HttpRule::default()
            }],
            ..ntgw_ir::HttpRoute::default()
        }],
        backends: vec![BackendCluster {
            ai_service: None,
            token_policy: None,
            name: "orders:8080".to_string().into(),
            namespace: "default".to_string().into(),
            protocol: "HTTP".to_string().into(),
            endpoints: vec![BackendEndpoint {
                address: "10.0.0.10".to_string(),
                port: 8080,
                healthy: true,
            }],
            wasm_plugin: None,
        
                circuit_breaker: None,
                security_policy: None,}],
        ..Snapshot::default()
    };
    snapshot.rebuild_runtime_indexes();
    let selected = snapshot
        .select_backend(&ntgw_ir::RequestMeta::new(
            Some("orders.example.com".to_string()),
            "/",
            "GET",
            BTreeMap::new(),
        ))
        .expect("selected backend");
    let cache = SelectedBackendConfigCache;

    let first = selected_backend_config_cached(&cache, &snapshot, &selected)
        .expect("selected backend config");
    let second = selected_backend_config_cached(&cache, &snapshot, &selected)
        .expect("selected backend config");

    assert!(std::sync::Arc::ptr_eq(&first, &second));
}

#[test]
fn selected_backend_config_cached_for_fast_path_uses_compiled_runtime_ids() {
    let mut snapshot = Snapshot {
        id: "snapshot-fast".to_string(),
        listeners: vec![ntgw_ir::Listener {
            name: "default/gw/http".to_string(),
            address: "0.0.0.0".to_string(),
            port: 80,
            protocol: "LISTENER_PROTOCOL_HTTP".to_string(),
            attached_routes: vec!["default/orders".to_string()],
            ..ntgw_ir::Listener::default()
        }],
        http_routes: vec![ntgw_ir::HttpRoute {
            name: "orders".to_string(),
            namespace: "default".to_string(),
            hostnames: vec!["orders.example.com".to_string()],
            rules: vec![ntgw_ir::HttpRule {
                name: String::new(),
                backend_refs: vec![ntgw_ir::BackendRef {
                    namespace: "default".to_string(),
                    name: "orders".to_string(),
                    port: 8080,
                    ..ntgw_ir::BackendRef::default()
                }],
                ..ntgw_ir::HttpRule::default()
            }],
            ..ntgw_ir::HttpRoute::default()
        }],
        backends: vec![BackendCluster {
            ai_service: None,
            token_policy: None,
            name: "orders:8080".to_string().into(),
            namespace: "default".to_string().into(),
            protocol: "HTTP".to_string().into(),
            endpoints: vec![BackendEndpoint {
                address: "10.0.0.10".to_string(),
                port: 8080,
                healthy: true,
            }],
            wasm_plugin: None,
        
                circuit_breaker: None,
                security_policy: None,}],
        ..Snapshot::default()
    };
    snapshot.rebuild_runtime_indexes();
    let selected = snapshot
        .select_http_fast_path(ntgw_ir::HttpFastPathRequest {
            host: Some("orders.example.com"),
            port: 80,
            path: "/",
            method: "GET",
            is_grpc: false,
        })
        .expect("fast selected backend");
    let cache = SelectedBackendConfigCache;

    let config = selected_backend_config_cached_for_fast_path(&cache, &snapshot, &selected)
        .expect("fast path selected backend config");

    assert_eq!(config.runtime_ids, selected.runtime_ids);
    assert_eq!(config.peer_port, selected.backend.port as u16);
}

#[test]
fn selected_backend_config_cache_invalidates_when_snapshot_id_changes() {
    let mut snapshot = Snapshot {
        id: "snapshot-one".to_string(),
        listeners: vec![ntgw_ir::Listener {
            name: "default/gw/http".to_string(),
            address: "0.0.0.0".to_string(),
            port: 80,
            protocol: "LISTENER_PROTOCOL_HTTP".to_string(),
            attached_routes: vec!["default/orders".to_string()],
            ..ntgw_ir::Listener::default()
        }],
        http_routes: vec![ntgw_ir::HttpRoute {
            name: "orders".to_string(),
            namespace: "default".to_string(),
            hostnames: vec!["orders.example.com".to_string()],
            rules: vec![ntgw_ir::HttpRule {
                name: String::new(),
                backend_refs: vec![ntgw_ir::BackendRef {
                    namespace: "default".to_string(),
                    name: "orders".to_string(),
                    port: 8080,
                    ..ntgw_ir::BackendRef::default()
                }],
                ..ntgw_ir::HttpRule::default()
            }],
            ..ntgw_ir::HttpRoute::default()
        }],
        backends: vec![BackendCluster {
            ai_service: None,
            token_policy: None,
            name: "orders:8080".to_string().into(),
            namespace: "default".to_string().into(),
            protocol: "HTTP".to_string().into(),
            endpoints: vec![BackendEndpoint {
                address: "10.0.0.10".to_string(),
                port: 8080,
                healthy: true,
            }],
            wasm_plugin: None,
        
                circuit_breaker: None,
                security_policy: None,}],
        ..Snapshot::default()
    };
    snapshot.rebuild_runtime_indexes();
    let selected = snapshot
        .select_backend(&ntgw_ir::RequestMeta::new(
            Some("orders.example.com".to_string()),
            "/",
            "GET",
            BTreeMap::new(),
        ))
        .expect("selected backend");
    let cache = SelectedBackendConfigCache;

    let first = selected_backend_config_cached(&cache, &snapshot, &selected)
        .expect("selected backend config");
    snapshot.id = "snapshot-two".to_string();
    let second = selected_backend_config_cached(&cache, &snapshot, &selected)
        .expect("selected backend config");

    assert!(!std::sync::Arc::ptr_eq(&first, &second));
}

#[test]
fn selected_backend_config_precomputes_traffic_topology() {
    let snapshot = Snapshot::default();
    let selected = SelectedBackend { route_policy: None,
        route_kind: RouteKind::Http,
        route_name: "orders".to_string(),
        route_namespace: "default".to_string(),
        rule_index: None,
        route_annotations: BTreeMap::new(),
        listener_name: "default/gw/http".to_string(),
        listener_protocol: "LISTENER_PROTOCOL_HTTP".to_string(),
        backend: BackendEndpoint {
            address: "10.0.0.10".to_string(),
            port: 8080,
            healthy: true,
        },
        backend_name: "default/orders:8080".to_string(),
        filters: Vec::new(),
        matched_http_path: None,
        timeouts: None,
        retry: None,
        session_persistence: None,
        backend_tls: None,
    };

    let config = selected_backend_config(&snapshot, &selected).expect("selected backend config");

    assert_eq!(
        config.traffic_topology.listener_node_id,
        "listener:default/gw/http"
    );
    assert_eq!(
        config.traffic_topology.route_node_id,
        "route:HTTPRoute:default/orders"
    );
    assert_eq!(
        config.traffic_topology.backend_node_id.as_deref(),
        Some("backend:default/orders:8080")
    );
    assert_eq!(
        config.traffic_topology.route_to_backend_edge_id.as_deref(),
        Some("edge:route:HTTPRoute:default/orders:backend:default/orders:8080")
    );
}

#[test]
fn selected_backend_config_precomputes_tls_validation_and_client_cert_handles() {
    let snapshot = Snapshot {
        backends: vec![BackendCluster {
            ai_service: None,
            token_policy: None,
            name: "orders:8443".to_string().into(),
            namespace: "default".to_string().into(),
            protocol: "HTTPS".to_string().into(),
            endpoints: vec![BackendEndpoint {
                address: "127.0.0.1".to_string(),
                port: 8443,
                healthy: true,
            }],
            wasm_plugin: None,
        
                circuit_breaker: None,
                security_policy: None,}],
        backend_policies: BTreeMap::from([(
            "default/orders:8443".to_string(),
            BackendPolicy {
                connect_timeout: None,
                request_timeout: None,
                tls_validation: Some(BackendTlsValidation {
                    hostname: "orders.internal.example".to_string(),
                    use_system_ca_certificates: true,
                    ca_pems: Vec::new(),
                    subject_alt_names: Vec::new(),
                    min_version: String::new(),
                    max_version: String::new(),
                }),
                session_persistence: None,
                load_balancing: None,
                health_check: None,
                outlier_detection: None,
            },
        )]),
        secrets: vec![ntgw_ir::SecretMaterial {
            namespace: "default".to_string(),
            name: "client-cert".to_string(),
            cert_pem: TEST_CLIENT_CERT_PEM.to_string(),
            key_pem: TEST_CLIENT_KEY_PEM.to_string(),
        htpasswd: String::new(),
                    oidc_client_secret: String::new(),
        }],
        ..Snapshot::default()
    };
    let selected = SelectedBackend { route_policy: None,
        route_kind: RouteKind::Http,
        route_name: "route".to_string(),
        route_namespace: "default".to_string(),
        rule_index: None,
        route_annotations: BTreeMap::new(),
        listener_name: "default/gw/https".to_string(),
        listener_protocol: "LISTENER_PROTOCOL_HTTPS".to_string(),
        backend: BackendEndpoint {
            address: "127.0.0.1".to_string(),
            port: 8443,
            healthy: true,
        },
        backend_name: "default/orders:8443".to_string(),
        filters: Vec::new(),
        matched_http_path: None,
        timeouts: None,
        retry: None,
        session_persistence: None,
        backend_tls: Some(BackendTlsConfig {
            client_certificate_ref: "default/client-cert".to_string(),
        }),
    };

    let config = selected_backend_config(&snapshot, &selected).expect("selected backend config");

    let tls_validation = config
        .backend_tls_validation
        .as_ref()
        .expect("cached backend TLS validation");
    assert!(tls_validation.group_key > 0);
    assert!(config.client_cert_key.is_some());

    let peer = build_upstream_peer_with_cached_config(&selected, &config, None, &UpstreamTuningOptions::default()).expect("peer");

    assert!(peer.is_tls());
    assert_eq!(peer.sni, "orders.internal.example");
    assert_eq!(peer.group_key, tls_validation.group_key);
    assert!(peer.options.verify_cert);
    assert!(peer.client_cert_key.is_some());
}

#[test]
fn selected_backend_config_isolated_for_same_backend_across_route_overrides() {
    let snapshot = Snapshot {
        secrets: vec![ntgw_ir::SecretMaterial {
            namespace: "default".to_string(),
            name: "client-cert".to_string(),
            cert_pem: TEST_CLIENT_CERT_PEM.to_string(),
            key_pem: TEST_CLIENT_KEY_PEM.to_string(),
        htpasswd: String::new(),
                    oidc_client_secret: String::new(),
        }],
        ..Snapshot::default()
    };
    let endpoint = BackendEndpoint {
        address: "127.0.0.1".to_string(),
        port: 8443,
        healthy: true,
    };
    let plain_route = selected_backend_for_cached_config_route(
        "plain-route",
        RouteKind::Http,
        endpoint.clone(),
        None,
        None,
    );
    let secure_route = selected_backend_for_cached_config_route(
        "secure-route",
        RouteKind::Grpc,
        endpoint,
        Some(RouteTimeouts {
            request: None,
            backend_request: Some(std::time::Duration::from_secs(2)),
            connect: None,
            next_upstream: None,
        }),
        Some(BackendTlsConfig {
            client_certificate_ref: "default/client-cert".to_string(),
        }),
    );
    let plain_policy = BackendPolicy {
        connect_timeout: Some(std::time::Duration::from_secs(1)),
        request_timeout: Some(std::time::Duration::from_secs(5)),
        tls_validation: None,
        session_persistence: None,
        load_balancing: None,
        health_check: None,
        outlier_detection: None,
    };
    let secure_policy = BackendPolicy {
        connect_timeout: Some(std::time::Duration::from_secs(9)),
        request_timeout: Some(std::time::Duration::from_secs(11)),
        tls_validation: Some(BackendTlsValidation {
            hostname: "secure.internal.example".to_string(),
            use_system_ca_certificates: true,
            ca_pems: Vec::new(),
            subject_alt_names: Vec::new(),
            min_version: String::new(),
            max_version: String::new(),
        }),
        session_persistence: None,
        load_balancing: None,
        health_check: None,
        outlier_detection: None,
    };

    let plain_config = selected_backend_config_with_overrides(
        &snapshot,
        &plain_route,
        Some("HTTP"),
        Some(&plain_policy),
    )
    .expect("plain selected backend config");
    let secure_config = selected_backend_config_with_overrides(
        &snapshot,
        &secure_route,
        Some("GRPCS"),
        Some(&secure_policy),
    )
    .expect("secure selected backend config");

    assert!(!plain_config.tls_enabled);
    assert!(!plain_config.use_http2);
    assert_eq!(plain_config.sni, "");
    assert_eq!(
        plain_config.connect_timeout,
        Some(std::time::Duration::from_secs(1))
    );
    assert_eq!(
        plain_config.request_timeout,
        Some(std::time::Duration::from_secs(5))
    );
    assert!(plain_config.backend_tls_validation.is_none());
    assert!(plain_config.client_cert_key.is_none());

    assert!(secure_config.tls_enabled);
    assert!(secure_config.use_http2);
    assert_eq!(secure_config.sni, "secure.internal.example");
    assert_eq!(
        secure_config.connect_timeout,
        Some(std::time::Duration::from_secs(9))
    );
    assert_eq!(
        secure_config.request_timeout,
        Some(std::time::Duration::from_secs(2))
    );
    assert!(secure_config.backend_tls_validation.is_some());
    assert!(secure_config.client_cert_key.is_some());

    let plain_peer = build_upstream_peer_with_cached_config(&plain_route, &plain_config, None, &UpstreamTuningOptions::default())
        .expect("plain peer");
    let secure_peer = build_upstream_peer_with_cached_config(&secure_route, &secure_config, None, &UpstreamTuningOptions::default())
        .expect("secure peer");

    assert!(!plain_peer.is_tls());
    assert_eq!(plain_peer.options.alpn.get_max_http_version(), 1);
    assert!(secure_peer.is_tls());
    assert_eq!(secure_peer.options.alpn.get_max_http_version(), 2);
    assert_eq!(secure_peer.sni, "secure.internal.example");
    assert_ne!(
        plain_peer.options.connection_timeout,
        secure_peer.options.connection_timeout
    );
    assert_ne!(
        plain_peer.options.read_timeout,
        secure_peer.options.read_timeout
    );
    assert!(plain_peer.client_cert_key.is_none());
    assert!(secure_peer.client_cert_key.is_some());
}

fn selected_backend_for_cached_config_route(
    route_name: &str,
    route_kind: RouteKind,
    endpoint: BackendEndpoint,
    timeouts: Option<RouteTimeouts>,
    backend_tls: Option<BackendTlsConfig>,
) -> SelectedBackend {
    SelectedBackend { route_policy: None,
        route_kind,
        route_name: route_name.to_string(),
        route_namespace: "default".to_string(),
        rule_index: None,
        route_annotations: BTreeMap::new(),
        listener_name: "default/gw/http".to_string(),
        listener_protocol: "LISTENER_PROTOCOL_HTTP".to_string(),
        backend: endpoint,
        backend_name: "default/orders:8443".to_string(),
        filters: Vec::new(),
        matched_http_path: None,
        timeouts,
        retry: None,
        session_persistence: None,
        backend_tls,
    }
}
