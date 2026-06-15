use ntgw_observability::AccessLogMode;

use super::super::super::request::{
    cache_access_log_request_headers_if_needed, cache_request_headers_for_filters_and_access_log,
};

#[test]
fn cache_access_log_request_headers_only_captures_named_nginx_headers() {
    let mut ctx = RequestContext::default();
    let headers = BTreeMap::from([
        ("user-agent".to_string(), vec!["curl/8.7.1".to_string()]),
        (
            "x-forwarded-for".to_string(),
            vec!["203.0.113.10".to_string()],
        ),
        ("cookie".to_string(), vec!["session=1".to_string()]),
    ]);
    let access_log = AccessLogOptions {
        enabled: true,
        mode: AccessLogMode::Text,
        format: r#"$remote_addr "$http_user_agent" "$http_x_forwarded_for""#.to_string(),
        ..AccessLogOptions::default()
    };

    cache_access_log_request_headers_if_needed(&mut ctx, &headers, &access_log, &BTreeMap::new());

    assert_eq!(
        ctx.access_log_request_headers,
        BTreeMap::from([
            ("user-agent".to_string(), "curl/8.7.1".to_string()),
            (
                "x-forwarded-for".to_string(),
                "203.0.113.10".to_string(),
            ),
        ])
    );
    assert!(ctx.request_headers.is_none());
}

#[test]
fn cache_access_log_request_headers_skips_templates_without_http_variables() {
    let mut ctx = RequestContext::default();
    let headers = BTreeMap::from([(
        "user-agent".to_string(),
        vec!["curl/8.7.1".to_string()],
    )]);
    let access_log = AccessLogOptions {
        enabled: true,
        mode: AccessLogMode::Text,
        format: "$remote_addr $status $request_time".to_string(),
        ..AccessLogOptions::default()
    };

    cache_access_log_request_headers_if_needed(&mut ctx, &headers, &access_log, &BTreeMap::new());

    assert!(ctx.access_log_request_headers.is_empty());
}

#[test]
fn cache_request_headers_for_filters_and_access_log_keeps_stores_separate() {
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
        ("origin".to_string(), vec!["https://example.com".to_string()]),
        ("user-agent".to_string(), vec!["curl/8.7.1".to_string()]),
    ]);
    let access_log = AccessLogOptions {
        enabled: true,
        mode: AccessLogMode::Text,
        format: r#"$remote_addr "$http_user_agent""#.to_string(),
        ..AccessLogOptions::default()
    };

    cache_request_headers_for_filters_and_access_log(
        &mut ctx,
        &headers,
        &filters,
        &access_log,
        &BTreeMap::new(),
    );

    assert_eq!(
        ctx.request_headers,
        Some(BTreeMap::from([(
            "origin".to_string(),
            vec!["https://example.com".to_string()],
        )]))
    );
    assert_eq!(
        ctx.access_log_request_headers,
        BTreeMap::from([("user-agent".to_string(), "curl/8.7.1".to_string())])
    );
}
