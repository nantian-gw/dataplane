fn tcproute_listener(
    name: &str,
    protocol: &str,
    port: u32,
    attached_routes: Vec<&str>,
) -> Listener {
    Listener {
        name: name.to_string(),
        address: "0.0.0.0".to_string(),
        addresses: vec!["0.0.0.0".to_string()],
        port,
        protocol: protocol.to_string(),
        hostnames: vec![],
        attached_routes: attached_routes.into_iter().map(str::to_string).collect(),
        tls: None,
        backend_tls: None,
        metadata: BTreeMap::new(),
    }
}

fn tcproute_route(
    name: &str,
    matches: Vec<StreamMatch>,
    backend_refs: Vec<BackendRef>,
) -> StreamRoute {
    StreamRoute {
        name: name.to_string(),
        namespace: "default".to_string(),
        kind: "ROUTE_KIND_TCP".to_string(),
        parent_refs: vec![],
        rules: vec![StreamRule {
            name: String::new(),
            matches,
            backend_refs,
        }],
        labels: BTreeMap::new(),
            annotations: BTreeMap::new(),
    }
}

fn tcproute_backend(name: &str, port: u32, endpoints: Vec<BackendEndpoint>) -> BackendCluster {
    BackendCluster {
        name: format!("{name}:{port}"),
        namespace: "default".to_string(),
        protocol: "TCP".to_string(),
        endpoints,
        wasm_plugin: None,
                ai_service: None,
                token_policy: None,
        circuit_breaker: None,
    }
}

#[test]
fn tcproute_selects_backend_by_attached_tcp_listener() {
    let mut snapshot = Snapshot {
        listeners: vec![tcproute_listener(
            "default/gw/tcp",
            "LISTENER_PROTOCOL_TCP",
            9000,
            vec!["default/tcp-echo"],
        )],
        stream_routes: vec![tcproute_route(
            "tcp-echo",
            vec![StreamMatch::default()],
            vec![backend_ref("default", "echo", 8080)],
        )],
        backends: vec![tcproute_backend(
            "echo",
            8080,
            vec![BackendEndpoint {
                address: "10.0.0.10".to_string(),
                port: 8080,
                healthy: true,
            }],
        )],
        ..Snapshot::default()
    };
    snapshot.rebuild_runtime_indexes();

    let selected = snapshot
        .select_stream_backend("default/gw/tcp", None)
        .expect("tcp backend");

    assert_eq!(selected.route_kind, RouteKind::Tcp);
    assert_eq!(selected.route_namespace, "default");
    assert_eq!(selected.route_name, "tcp-echo");
    assert_eq!(selected.listener_name, "default/gw/tcp");
    assert_eq!(selected.backend.address, "10.0.0.10");
}

#[test]
fn tcproute_does_not_select_unattached_route() {
    let mut snapshot = Snapshot {
        listeners: vec![tcproute_listener(
            "default/gw/tcp",
            "LISTENER_PROTOCOL_TCP",
            9000,
            vec![],
        )],
        stream_routes: vec![tcproute_route(
            "tcp-echo",
            vec![StreamMatch::default()],
            vec![backend_ref("default", "echo", 8080)],
        )],
        backends: vec![tcproute_backend(
            "echo",
            8080,
            vec![BackendEndpoint {
                address: "10.0.0.10".to_string(),
                port: 8080,
                healthy: true,
            }],
        )],
        ..Snapshot::default()
    };
    snapshot.rebuild_runtime_indexes();

    assert!(snapshot.select_stream_backend("default/gw/tcp", None).is_none());
}

#[test]
fn tcproute_does_not_match_udp_listener_even_when_attached() {
    let mut snapshot = Snapshot {
        listeners: vec![tcproute_listener(
            "default/gw/udp",
            "LISTENER_PROTOCOL_UDP",
            9000,
            vec!["default/tcp-echo"],
        )],
        stream_routes: vec![tcproute_route(
            "tcp-echo",
            vec![StreamMatch::default()],
            vec![backend_ref("default", "echo", 8080)],
        )],
        backends: vec![tcproute_backend(
            "echo",
            8080,
            vec![BackendEndpoint {
                address: "10.0.0.10".to_string(),
                port: 8080,
                healthy: true,
            }],
        )],
        ..Snapshot::default()
    };
    snapshot.rebuild_runtime_indexes();

    assert!(snapshot.select_stream_backend("default/gw/udp", None).is_none());
}

#[test]
fn tcproute_rule_port_must_match_listener_port() {
    let mut snapshot = Snapshot {
        listeners: vec![tcproute_listener(
            "default/gw/tcp",
            "LISTENER_PROTOCOL_TCP",
            9000,
            vec!["default/tcp-echo"],
        )],
        stream_routes: vec![tcproute_route(
            "tcp-echo",
            vec![StreamMatch {
                port: 9001,
                sni_hostname: String::new(),
                mode: TlsRouteMode::default(),
            }],
            vec![backend_ref("default", "echo", 8080)],
        )],
        backends: vec![tcproute_backend(
            "echo",
            8080,
            vec![BackendEndpoint {
                address: "10.0.0.10".to_string(),
                port: 8080,
                healthy: true,
            }],
        )],
        ..Snapshot::default()
    };
    snapshot.rebuild_runtime_indexes();

    assert!(snapshot.select_stream_backend("default/gw/tcp", None).is_none());
}

#[test]
fn tcproute_returns_none_for_missing_backend_cluster() {
    let mut snapshot = Snapshot {
        listeners: vec![tcproute_listener(
            "default/gw/tcp",
            "LISTENER_PROTOCOL_TCP",
            9000,
            vec!["default/tcp-missing"],
        )],
        stream_routes: vec![tcproute_route(
            "tcp-missing",
            vec![StreamMatch::default()],
            vec![backend_ref("default", "missing", 8080)],
        )],
        ..Snapshot::default()
    };
    snapshot.rebuild_runtime_indexes();

    assert!(snapshot.select_stream_backend("default/gw/tcp", None).is_none());
}

#[test]
fn tcproute_selects_only_healthy_backend_endpoint() {
    let mut snapshot = Snapshot {
        listeners: vec![tcproute_listener(
            "default/gw/tcp",
            "LISTENER_PROTOCOL_TCP",
            9000,
            vec!["default/tcp-echo"],
        )],
        stream_routes: vec![tcproute_route(
            "tcp-echo",
            vec![StreamMatch::default()],
            vec![backend_ref("default", "echo", 8080)],
        )],
        backends: vec![tcproute_backend(
            "echo",
            8080,
            vec![
                BackendEndpoint {
                    address: "10.0.0.11".to_string(),
                    port: 8080,
                    healthy: false,
                },
                BackendEndpoint {
                    address: "10.0.0.12".to_string(),
                    port: 8080,
                    healthy: true,
                },
            ],
        )],
        ..Snapshot::default()
    };
    snapshot.rebuild_runtime_indexes();

    let selected = snapshot
        .select_stream_backend("default/gw/tcp", None)
        .expect("tcp backend");

    assert_eq!(selected.backend.address, "10.0.0.12");
}
