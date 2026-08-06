#[test]
fn build_request_meta_uses_uri_authority_for_http2_requests() {
    let mut req = RequestHeader::build("POST", b"/svc.Method/Echo", None).expect("request header");
    req.set_uri(
        "http://grpc.example.com:80/svc.Method/Echo"
            .parse()
            .expect("uri"),
    );
    req.insert_header("content-type", "application/grpc")
        .expect("content-type");

    let meta = build_request_meta_from_header(&req);
    assert_eq!(meta.host.as_deref(), Some("grpc.example.com"));
    assert_eq!(meta.port, 80);
    assert_eq!(meta.path, "/svc.Method/Echo");
    assert_eq!(meta.method, "POST");
    assert!(meta.headers.is_empty());
}

#[test]
fn build_request_meta_skips_headers_by_default() {
    let mut req =
        RequestHeader::build("POST", b"/helloworld.Greeter/SayHello", None).expect("request");
    req.set_uri(
        "http://grpc.example.com/helloworld.Greeter/SayHello"
            .parse()
            .expect("uri"),
    );
    req.insert_header("content-type", "application/grpc+proto")
        .expect("content-type");
    req.insert_header("te", "trailers").expect("te");
    req.insert_header("grpc-timeout", "150m")
        .expect("grpc-timeout");
    req.insert_header("x-tenant", "blue").expect("x-tenant");
    req.append_header("x-tenant", "green")
        .expect("x-tenant append");
    req.insert_header("grpc-trace-bin", "trace-token")
        .expect("grpc-trace-bin");

    let meta = build_request_meta_from_header(&req);

    assert_eq!(meta.host.as_deref(), Some("grpc.example.com"));
    assert_eq!(meta.path, "/helloworld.Greeter/SayHello");
    assert!(meta.headers.is_empty(), "headers are lazy by default");
}

#[test]
fn build_request_meta_prefers_host_header_over_uri_authority() {
    let mut req = RequestHeader::build("GET", b"/", None).expect("request header");
    req.set_uri("http://authority.example.com/".parse().expect("uri"));
    req.insert_header("host", "header.example.com:8080")
        .expect("host header");

    let meta = build_request_meta_from_header(&req);
    assert_eq!(meta.host.as_deref(), Some("header.example.com"));
    assert_eq!(meta.port, 8080);
}

#[test]
fn build_selection_request_meta_omits_headers_when_not_required() {
    let mut req = RequestHeader::build("GET", b"/orders?debug=false", None)
        .expect("request header");
    req.insert_header("host", "api.example.com")
        .expect("host header");
    req.insert_header("x-tenant", "blue").expect("tenant");
    req.insert_header("x-request-id", "req-1")
        .expect("request id");

    let meta = build_selection_request_meta_from_header_with_port(&req, 8080, None, false);

    assert_eq!(meta.host.as_deref(), Some("api.example.com"));
    assert_eq!(meta.port, 8080);
    assert_eq!(meta.path, "/orders");
    assert!(meta.headers.is_empty());
    assert_eq!(
        meta.query_params.get("debug"),
        Some(&vec!["false".to_string()])
    );
}

#[test]
fn build_selection_request_meta_keeps_grpc_content_type_without_full_headers() {
    let mut req = RequestHeader::build("POST", b"/pkg.Service/Call", None)
        .expect("request header");
    req.insert_header("content-type", "application/grpc+proto")
        .expect("content-type");
    req.insert_header("x-tenant", "blue").expect("tenant");

    let meta = build_selection_request_meta_from_header_with_port(&req, 8080, None, false);

    assert_eq!(
        meta.headers.get("content-type"),
        Some(&vec!["application/grpc+proto".to_string()])
    );
    assert!(!meta.headers.contains_key("x-tenant"));
}

#[test]
fn build_selection_request_meta_materializes_headers_when_required() {
    let mut req = RequestHeader::build("GET", b"/", None).expect("request header");
    req.insert_header("x-tenant", "blue").expect("tenant");

    let meta = build_selection_request_meta_from_header_with_port(&req, 8080, None, true);

    assert_eq!(
        meta.headers.get("x-tenant"),
        Some(&vec!["blue".to_string()])
    );
}
