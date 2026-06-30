#[test]
fn runtime_indexes_precompute_stream_listener_route_candidates() {
    let mut snapshot = Snapshot {
        listeners: vec![
            Listener {
                name: "default/gw/tcp".to_string().into(),
                port: 9000,
                protocol: "TCP".to_string().into(),
                attached_routes: vec!["default/tcp-a".to_string(), "default/tcp-b".to_string()],
                ..Listener::default()
            },
            Listener {
                name: "default/gw/tls".to_string().into(),
                port: 9443,
                protocol: "TLS".to_string().into(),
                attached_routes: vec!["default/tls-a".to_string()],
                ..Listener::default()
            },
        ],
        stream_routes: vec![
            StreamRoute {
                name: "tcp-a".to_string().into(),
                namespace: "default".to_string().into(),
                kind: "TCP".to_string(),
                ..StreamRoute::default()
            },
            StreamRoute {
                name: "tcp-b".to_string().into(),
                namespace: "default".to_string().into(),
                kind: "TCP".to_string(),
                ..StreamRoute::default()
            },
            StreamRoute {
                name: "tls-a".to_string().into(),
                namespace: "default".to_string().into(),
                kind: "TLS".to_string(),
                ..StreamRoute::default()
            },
        ],
        ..Snapshot::default()
    };

    snapshot.rebuild_runtime_indexes();

    assert_eq!(snapshot.listener_name_index.get("default/gw/tcp"), Some(&0));
    assert_eq!(snapshot.listener_name_index.get("default/gw/tls"), Some(&1));

    assert_eq!(
        snapshot.stream_listener_route_index.get("default/gw/tcp"),
        Some(&vec![0, 1])
    );
    assert_eq!(
        snapshot.stream_listener_route_index.get("default/gw/tls"),
        Some(&vec![2])
    );

    let mut visited_tcp_routes = Vec::new();
    snapshot.visit_stream_route_candidates(&snapshot.listeners[0], &RouteKind::Tcp, |route| {
        visited_tcp_routes.push(route.name.as_str());
        true
    });
    assert_eq!(visited_tcp_routes, vec!["tcp-a", "tcp-b"]);

    let mut early_stop_routes = Vec::new();
    snapshot.visit_stream_route_candidates(&snapshot.listeners[0], &RouteKind::Tcp, |route| {
        early_stop_routes.push(route.name.as_str());
        false
    });
    assert_eq!(early_stop_routes, vec!["tcp-a"]);
}

#[test]
fn stream_listener_set_best_candidates_visit_in_input_order() {
    let mut snapshot = Snapshot {
        listeners: vec![
            Listener {
                name: "default/gw/wild".to_string().into(),
                protocol: "TLS".to_string().into(),
                hostnames: vec!["*.example.com".to_string()],
                ..Listener::default()
            },
            Listener {
                name: "default/gw/exact-a".to_string().into(),
                protocol: "TLS".to_string().into(),
                hostnames: vec!["api.example.com".to_string()],
                ..Listener::default()
            },
            Listener {
                name: "default/gw/exact-b".to_string().into(),
                protocol: "TLS".to_string().into(),
                hostnames: vec!["api.example.com".to_string()],
                ..Listener::default()
            },
            Listener {
                name: "default/gw/catch-all".to_string().into(),
                protocol: "TLS".to_string().into(),
                ..Listener::default()
            },
        ],
        ..Snapshot::default()
    };
    snapshot.rebuild_runtime_indexes();

    let listener_names = vec![
        "default/gw/missing".to_string(),
        "default/gw/wild".to_string(),
        "default/gw/exact-a".to_string(),
        "default/gw/catch-all".to_string(),
        "default/gw/exact-b".to_string(),
    ];

    let mut visited = Vec::new();
    snapshot.visit_best_stream_listeners(&listener_names, Some("api.example.com"), |listener| {
        visited.push(listener.name.as_str());
        true
    });
    assert_eq!(visited, vec!["default/gw/exact-a", "default/gw/exact-b"]);

    let mut early_stop = Vec::new();
    snapshot.visit_best_stream_listeners(&listener_names, Some("api.example.com"), |listener| {
        early_stop.push(listener.name.as_str());
        false
    });
    assert_eq!(early_stop, vec!["default/gw/exact-a"]);
}
