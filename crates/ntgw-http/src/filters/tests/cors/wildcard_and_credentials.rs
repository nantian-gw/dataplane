use super::*;

#[test]
fn apply_response_filters_matches_gateway_api_origin_wildcards() {
    let mut response = ResponseHeader::build(200, None).expect("response");
    let request_headers = BTreeMap::from([(
        "origin".to_string(),
        vec!["https://xpto.www.bar.com".to_string()],
    )]);

    super::super::super::apply_response_filters(
        &mut response,
        &[Filter {
            filter_type: "CORS".to_string(),
            cors: Some(CorsFilter {
                allow_origins: vec!["https://*.bar.com".to_string()],
                allow_credentials: true,
                ..CorsFilter::default()
            }),
            ..Filter::default()
        }],
        Some("GET"),
        Some(&request_headers),
    )
    .expect("apply cors filters");

    assert_eq!(
        response
            .headers
            .get("access-control-allow-origin")
            .and_then(|value| value.to_str().ok()),
        Some("https://xpto.www.bar.com")
    );
}

#[test]
fn apply_response_filters_echoes_wildcard_origin_when_credentials_are_allowed() {
    let mut response = ResponseHeader::build(200, None).expect("response");
    let request_headers = BTreeMap::from([(
        "origin".to_string(),
        vec!["https://other.foo.com".to_string()],
    )]);

    super::super::super::apply_response_filters(
        &mut response,
        &[Filter {
            filter_type: "CORS".to_string(),
            cors: Some(CorsFilter {
                allow_origins: vec!["*".to_string()],
                allow_credentials: true,
                ..CorsFilter::default()
            }),
            ..Filter::default()
        }],
        Some("GET"),
        Some(&request_headers),
    )
    .expect("apply cors filters");

    assert_eq!(
        response
            .headers
            .get("access-control-allow-origin")
            .and_then(|value| value.to_str().ok()),
        Some("https://other.foo.com")
    );
    assert_eq!(
        response
            .headers
            .get("access-control-allow-credentials")
            .and_then(|value| value.to_str().ok()),
        Some("true")
    );
}

#[test]
fn apply_response_filters_echoes_wildcard_origin_for_cookie_requests_without_credentials_header() {
    let mut response = ResponseHeader::build(200, None).expect("response");
    let request_headers = BTreeMap::from([
        (
            "origin".to_string(),
            vec!["https://other.foo.com".to_string()],
        ),
        ("cookie".to_string(), vec!["session=1".to_string()]),
    ]);

    super::super::super::apply_response_filters(
        &mut response,
        &[Filter {
            filter_type: "CORS".to_string(),
            cors: Some(CorsFilter {
                allow_origins: vec!["*".to_string()],
                allow_credentials: false,
                ..CorsFilter::default()
            }),
            ..Filter::default()
        }],
        Some("GET"),
        Some(&request_headers),
    )
    .expect("apply cors filters");

    assert_eq!(
        response
            .headers
            .get("access-control-allow-origin")
            .and_then(|value| value.to_str().ok()),
        Some("https://other.foo.com")
    );
    assert!(
        response
            .headers
            .get("access-control-allow-credentials")
            .is_none()
    );
}

#[test]
fn apply_response_filters_echoes_wildcard_methods_and_headers_when_credentials_are_allowed() {
    let mut response = ResponseHeader::build(204, None).expect("response");
    let request_headers = BTreeMap::from([
        (
            "origin".to_string(),
            vec!["https://other.foo.com".to_string()],
        ),
        (
            "access-control-request-method".to_string(),
            vec!["PUT".to_string()],
        ),
        (
            "access-control-request-headers".to_string(),
            vec!["x-header-1, x-header-2".to_string()],
        ),
    ]);

    super::super::super::apply_response_filters(
        &mut response,
        &[Filter {
            filter_type: "CORS".to_string(),
            cors: Some(CorsFilter {
                allow_origins: vec!["*".to_string()],
                allow_methods: vec!["*".to_string()],
                allow_headers: vec!["*".to_string()],
                allow_credentials: true,
                ..CorsFilter::default()
            }),
            ..Filter::default()
        }],
        Some("OPTIONS"),
        Some(&request_headers),
    )
    .expect("apply cors filters");

    assert_eq!(
        response
            .headers
            .get("access-control-allow-origin")
            .and_then(|value| value.to_str().ok()),
        Some("https://other.foo.com")
    );
    assert_eq!(
        response
            .headers
            .get("access-control-allow-methods")
            .and_then(|value| value.to_str().ok()),
        Some("PUT")
    );
    assert_eq!(
        response
            .headers
            .get("access-control-allow-headers")
            .and_then(|value| value.to_str().ok()),
        Some("x-header-1, x-header-2")
    );
}
