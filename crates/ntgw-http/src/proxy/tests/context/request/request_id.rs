#[test]
fn request_id_from_headers_uses_documented_priority_order() {
    let mut headers = BTreeMap::new();
    headers.insert("grpc-trace-bin".to_string(), vec!["grpc-trace".to_string()]);
    headers.insert("traceparent".to_string(), vec!["trace-parent".to_string()]);
    headers.insert(
        "x-correlation-id".to_string(),
        vec!["correlation".to_string()],
    );
    headers.insert("x-request-id".to_string(), vec!["request-id".to_string()]);

    assert_eq!(request_id_from_headers(&headers), "request-id");

    headers.remove("x-request-id");
    assert_eq!(request_id_from_headers(&headers), "correlation");

    headers.remove("x-correlation-id");
    assert_eq!(request_id_from_headers(&headers), "trace-parent");

    headers.remove("traceparent");
    assert_eq!(request_id_from_headers(&headers), "grpc-trace");
}
