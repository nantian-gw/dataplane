#[test]
fn cache_request_headers_only_for_filters_that_need_them() {
    let mut ctx = RequestContext::default();
    let headers = BTreeMap::from([(
        "origin".to_string(),
        vec!["https://example.com".to_string()],
    )]);

    let request_header_filters = vec![Filter {
        filter_type: "RequestHeaderModifier".to_string(),
        ..Filter::default()
    }];
    cache_request_headers_if_needed(&mut ctx, &headers, &request_header_filters);
    assert!(ctx.request_headers.is_none());

    let cors_filters = vec![Filter {
        filter_type: "CORS".to_string(),
        cors: Some(CorsFilter {
            allow_origins: vec!["https://example.com".to_string()],
            allow_credentials: true,
            ..CorsFilter::default()
        }),
        ..Filter::default()
    }];
    cache_request_headers_if_needed(&mut ctx, &headers, &cors_filters);
    assert_eq!(
        ctx.request_headers,
        Some(BTreeMap::from([(
            "origin".to_string(),
            vec!["https://example.com".to_string()],
        )]))
    );
}

#[test]
fn cache_request_headers_only_keeps_cors_inputs() {
    let mut ctx = RequestContext::default();
    let filters = vec![Filter {
        filter_type: "CORS".to_string(),
        cors: Some(CorsFilter {
            allow_origins: vec!["https://example.com".to_string()],
            ..CorsFilter::default()
        }),
        ..Filter::default()
    }];
    let headers = BTreeMap::from([
        (
            "origin".to_string(),
            vec!["https://example.com".to_string()],
        ),
        (
            "access-control-request-method".to_string(),
            vec!["POST".to_string()],
        ),
        (
            "access-control-request-headers".to_string(),
            vec!["authorization, content-type".to_string()],
        ),
        ("cookie".to_string(), vec!["session=1".to_string()]),
        ("x-request-id".to_string(), vec!["req-1".to_string()]),
    ]);

    cache_request_headers_if_needed(&mut ctx, &headers, &filters);

    assert_eq!(
        ctx.request_headers,
        Some(BTreeMap::from([
            (
                "access-control-request-method".to_string(),
                vec!["POST".to_string()],
            ),
            (
                "access-control-request-headers".to_string(),
                vec!["authorization, content-type".to_string()],
            ),
            (
                "origin".to_string(),
                vec!["https://example.com".to_string()],
            ),
            ("cookie".to_string(), vec!["session=1".to_string()]),
        ]))
    );
}
