use super::*;

#[test]
fn apply_response_filters_adds_cors_headers_for_matching_origin() {
    let mut response = ResponseHeader::build(200, None).expect("response");
    let request_headers = BTreeMap::from([(
        "origin".to_string(),
        vec!["https://app.example".to_string()],
    )]);

    super::super::super::apply_response_filters(
        &mut response,
        &[Filter {
            filter_type: "CORS".to_string(),
            cors: Some(CorsFilter {
                allow_origins: vec!["https://app.example".to_string()],
                allow_credentials: true,
                expose_headers: vec!["x-trace-id".to_string()],
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
        Some("https://app.example")
    );
    assert_eq!(
        response
            .headers
            .get("access-control-allow-credentials")
            .and_then(|value| value.to_str().ok()),
        Some("true")
    );
    assert_eq!(
        response
            .headers
            .get("access-control-expose-headers")
            .and_then(|value| value.to_str().ok()),
        Some("x-trace-id")
    );
}
