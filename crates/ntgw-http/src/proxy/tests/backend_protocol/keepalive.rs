#[test]
fn build_upstream_peer_applies_configured_tcp_keepalive() {
    let selected = SelectedBackend {
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
        backend_name: "default/echo:8080".to_string(),
        filters: Vec::new(),
        matched_http_path: None,
        timeouts: None,
        retry: None,
        session_persistence: None,
        backend_tls: None,
    };
    let keepalive = pingora::protocols::l4::ext::TcpKeepalive {
        idle: std::time::Duration::from_secs(30),
        interval: std::time::Duration::from_secs(10),
        count: 6,
        #[cfg(target_os = "linux")]
        user_timeout: std::time::Duration::from_secs(3),
    };

    let snapshot = Snapshot::default();
    let config = selected_backend_config_with_overrides(
        &snapshot,
        &selected,
        Some("HTTP"),
        None,
    )
    .expect("selected backend config");

    let peer = build_upstream_peer_with_keepalive(
        &snapshot,
        &selected,
        &config,
        Some(keepalive.clone()),
    )
    .expect("peer");

    let configured = peer
        .options
        .tcp_keepalive
        .as_ref()
        .expect("configured keepalive");
    assert_eq!(configured.idle, keepalive.idle);
    assert_eq!(configured.interval, keepalive.interval);
    assert_eq!(configured.count, keepalive.count);
    #[cfg(target_os = "linux")]
    assert_eq!(configured.user_timeout, keepalive.user_timeout);
}
