use super::*;

#[test]
fn apply_response_filters_adds_preflight_cors_headers() {
    let mut response = ResponseHeader::build(204, None).expect("response");
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

    super::super::super::apply_response_filters(
        &mut response,
        &[Filter {
            filter_type: "CORS".to_string(),
            cors: Some(CorsFilter {
                allow_origins: vec!["https://app.example".to_string()],
                allow_methods: vec!["GET".to_string(), "POST".to_string()],
                allow_headers: vec!["authorization".to_string(), "content-type".to_string()],
                max_age: Some(600),
                ..CorsFilter::default()
            }),
            ..Filter::default()
        }],
        Some("OPTIONS"),
        Some(&request_headers),
    )
    .expect("apply cors preflight filters");

    assert_eq!(
        response
            .headers
            .get("access-control-allow-methods")
            .and_then(|value| value.to_str().ok()),
        Some("GET, POST")
    );
    assert_eq!(
        response
            .headers
            .get("access-control-allow-headers")
            .and_then(|value| value.to_str().ok()),
        Some("authorization, content-type")
    );
    assert_eq!(
        response
            .headers
            .get("access-control-max-age")
            .and_then(|value| value.to_str().ok()),
        Some("600")
    );
}
