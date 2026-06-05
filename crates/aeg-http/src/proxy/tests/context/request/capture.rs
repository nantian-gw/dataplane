#[test]
fn capture_request_context_uses_request_fields() {
    let request = RequestMeta {
        source_ip: Some("192.0.2.10".to_string()),
        ..RequestMeta::new(
            Some("example.com".to_string()),
            "/orders?id=1",
            "POST",
            BTreeMap::new(),
        )
    };
    let mut ctx = RequestContext::default();

    capture_request_context(&mut ctx, &request);

    assert_eq!(ctx.client_ip, "192.0.2.10");
    assert_eq!(ctx.host, "example.com");
    assert_eq!(ctx.method, "POST");
    assert_eq!(ctx.path, "/orders");
}

#[test]
fn capture_request_context_overwrites_existing_buffers() {
    let request = RequestMeta::new(
        Some("example.com".to_string()),
        "/next",
        "PUT",
        BTreeMap::new(),
    );
    let mut ctx = RequestContext {
        client_ip: "stale-ip".to_string(),
        host: "stale-host".to_string(),
        method: "PATCH".to_string(),
        path: "/stale".to_string(),
        ..RequestContext::default()
    };

    capture_request_context(&mut ctx, &request);

    assert_eq!(ctx.client_ip, "-");
    assert_eq!(ctx.host, "example.com");
    assert_eq!(ctx.method, "PUT");
    assert_eq!(ctx.path, "/next");
}

#[test]
fn request_view_captures_context_and_materializes_when_needed() {
    let mut req = RequestHeader::build("POST", b"/orders?id=1", None).expect("request header");
    req.set_uri(
        "http://authority.example.com:8080/orders?id=1"
            .parse()
            .expect("uri"),
    );
    req.insert_header("host", "header.example.com:9443")
        .expect("host");
    req.insert_header("content-length", "123")
        .expect("content-length");
    req.insert_header("x-request-id", "req-123")
        .expect("request id");
    req.append_header("x-tenant", "blue").expect("tenant");
    req.append_header("x-tenant", "green").expect("tenant");

    let view = RequestView::from_header_with_port(&req, 0);
    let mut ctx = RequestContext::default();

    capture_request_context_from_view(&mut ctx, &view, Some("192.0.2.20"));

    assert_eq!(ctx.client_ip, "192.0.2.20");
    assert_eq!(ctx.host, "header.example.com");
    assert_eq!(ctx.method, "POST");
    assert_eq!(ctx.path, "/orders");
    assert_eq!(ctx.request_id, "req-123");
    assert_eq!(ctx.bytes_received, 0);
    assert_eq!(ctx.declared_request_body_bytes, 123);

    let meta = view.materialize();
    assert_eq!(meta.host.as_deref(), Some("header.example.com"));
    assert_eq!(meta.port, 9443);
    assert_eq!(meta.path, "/orders");
    assert_eq!(meta.query_params.get("id"), Some(&vec!["1".to_string()]));
    assert_eq!(
        meta.headers.get("x-tenant"),
        Some(&vec!["blue".to_string(), "green".to_string()])
    );
}

#[test]
fn request_view_minimal_capture_skips_observability_fields() {
    let mut req = RequestHeader::build("POST", b"/orders?id=1", None).expect("request header");
    req.set_uri(
        "http://authority.example.com:8080/orders?id=1"
            .parse()
            .expect("uri"),
    );
    req.insert_header("host", "header.example.com:9443")
        .expect("host");
    req.insert_header("content-length", "123")
        .expect("content-length");
    req.insert_header("x-request-id", "req-123")
        .expect("request id");

    let view = RequestView::from_header_with_port(&req, 0);
    let mut ctx = RequestContext {
        client_ip: "stale-ip".to_string(),
        host: "stale-host".to_string(),
        method: "GET".to_string(),
        path: "/stale".to_string(),
        request_id: "stale-request".to_string(),
        bytes_received: 99,
        declared_request_body_bytes: 99,
        ..RequestContext::default()
    };

    capture_request_context_from_view_for_features(&mut ctx, &view, Some("192.0.2.20"), false);

    assert!(ctx.client_ip.is_empty());
    assert!(ctx.host.is_empty());
    assert_eq!(ctx.method, "POST");
    assert!(ctx.path.is_empty());
    assert!(ctx.request_id.is_empty());
    assert_eq!(ctx.bytes_received, 0);
    assert_eq!(ctx.declared_request_body_bytes, 123);
}

#[test]
fn request_view_limit_capture_skips_declared_body_bytes_when_body_limit_disabled() {
    let mut req = RequestHeader::build("POST", b"/orders?id=1", None).expect("request header");
    req.insert_header("content-length", "123")
        .expect("content-length");

    let view = RequestView::from_header_with_port(&req, 0);
    let mut ctx = RequestContext {
        declared_request_body_bytes: 99,
        ..RequestContext::default()
    };

    capture_request_context_from_view_for_limits(&mut ctx, &view, None, false, false);

    assert_eq!(ctx.method, "POST");
    assert_eq!(ctx.bytes_received, 0);
    assert_eq!(ctx.declared_request_body_bytes, 0);
}

#[test]
fn request_header_bytes_for_limit_skips_header_scan_when_limit_disabled() {
    let mut req = RequestHeader::build("GET", b"/orders", None).expect("request header");
    req.insert_header("host", "example.com").expect("host");
    req.insert_header("x-large-header", "abcdef")
        .expect("large header");

    let view = RequestView::from_header_with_port(&req, 0);

    assert_eq!(request_header_bytes_for_limit(&view, 0), 0);
    assert!(request_header_bytes_for_limit(&view, 1) > 0);
}
