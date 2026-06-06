#[test]
fn apply_backend_protocol_uses_http2_for_grpc_routes() {
    let mut peer = HttpPeer::new(("127.0.0.1", 8080), false, String::new());

    apply_backend_protocol(&mut peer, &RouteKind::Grpc, None);

    assert_eq!(peer.options.alpn.get_min_http_version(), 2);
    assert_eq!(peer.options.alpn.get_max_http_version(), 2);
    assert_eq!(peer.options.max_h2_streams, DEFAULT_MAX_H2_UPSTREAM_STREAMS);
}

#[test]
fn apply_backend_protocol_uses_http2_for_h2c_backends() {
    let mut peer = HttpPeer::new(("127.0.0.1", 8080), false, String::new());

    apply_backend_protocol(&mut peer, &RouteKind::Http, Some("H2C"));

    assert_eq!(peer.options.alpn.get_min_http_version(), 2);
    assert_eq!(peer.options.alpn.get_max_http_version(), 2);
    assert_eq!(peer.options.max_h2_streams, DEFAULT_MAX_H2_UPSTREAM_STREAMS);
}

#[test]
fn apply_backend_protocol_keeps_http1_for_plain_http_backends() {
    let mut peer = HttpPeer::new(("127.0.0.1", 8080), false, String::new());

    apply_backend_protocol(&mut peer, &RouteKind::Http, Some("HTTP"));

    assert_eq!(peer.options.alpn.get_min_http_version(), 1);
    assert_eq!(peer.options.alpn.get_max_http_version(), 1);
}

#[test]
fn tls_backend_protocols_enable_tls_and_service_sni() {
    let tls_enabled = is_tls_backend_protocol(Some("HTTPS"));
    let sni = backend_tls_service_name("default/echo:8443").expect("service dns name");
    let peer = HttpPeer::new(("127.0.0.1", 8443), tls_enabled, sni.clone());

    assert!(peer.is_tls());
    assert_eq!(peer.sni, sni);
}

#[test]
fn backend_tls_server_name_uses_service_dns_shape() {
    assert_eq!(
        backend_tls_service_name("default/greeter:9090").as_deref(),
        Some("greeter.default.svc")
    );
    assert_eq!(backend_tls_service_name("malformed").as_deref(), None);
}
