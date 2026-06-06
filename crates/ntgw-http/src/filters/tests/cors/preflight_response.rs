use super::*;

#[test]
fn build_cors_preflight_response_returns_204_without_upstream_body() {
    let request_headers = BTreeMap::from([
        (
            "origin".to_string(),
            vec!["https://app.example".to_string()],
        ),
        (
            "access-control-request-method".to_string(),
            vec!["POST".to_string()],
        ),
    ]);

    let response = super::super::super::build_cors_preflight_response(
        &[Filter {
            filter_type: "CORS".to_string(),
            cors: Some(CorsFilter {
                allow_origins: vec!["https://app.example".to_string()],
                allow_methods: vec!["GET".to_string(), "POST".to_string()],
                ..CorsFilter::default()
            }),
            ..Filter::default()
        }],
        "OPTIONS",
        &request_headers,
    )
    .expect("build preflight response")
    .expect("preflight response");

    assert_eq!(response.status.as_u16(), 204);
    assert_eq!(
        response
            .headers
            .get("content-length")
            .and_then(|value| value.to_str().ok()),
        Some("0")
    );
    assert_eq!(
        response
            .headers
            .get("access-control-allow-methods")
            .and_then(|value| value.to_str().ok()),
        Some("GET, POST")
    );
}

#[test]
fn build_cors_preflight_response_ignores_non_options_requests() {
    let request_headers = BTreeMap::from([
        (
            "origin".to_string(),
            vec!["https://app.example".to_string()],
        ),
        (
            "access-control-request-method".to_string(),
            vec!["POST".to_string()],
        ),
    ]);

    let response = super::super::super::build_cors_preflight_response(
        &[Filter {
            filter_type: "CORS".to_string(),
            cors: Some(CorsFilter {
                allow_origins: vec!["https://app.example".to_string()],
                allow_methods: vec!["POST".to_string()],
                ..CorsFilter::default()
            }),
            ..Filter::default()
        }],
        "GET",
        &request_headers,
    )
    .expect("build preflight response");

    assert!(response.is_none());
}
