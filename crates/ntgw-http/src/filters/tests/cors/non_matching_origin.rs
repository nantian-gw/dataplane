use super::*;

#[test]
fn apply_response_filters_skips_cors_headers_for_non_matching_origin() {
    let mut response = ResponseHeader::build(200, None).expect("response");
    let request_headers = BTreeMap::from([(
        "origin".to_string(),
        vec!["https://other.example".to_string()],
    )]);

    super::super::super::apply_response_filters(
        &mut response,
        &[Filter {
            filter_type: "CORS".to_string(),
            cors: Some(CorsFilter {
                allow_origins: vec!["https://app.example".to_string()],
                allow_credentials: true,
                ..CorsFilter::default()
            }),
            ..Filter::default()
        }],
        Some("GET"),
        Some(&request_headers),
    )
    .expect("apply cors filters");

    assert!(
        response
            .headers
            .get("access-control-allow-origin")
            .is_none()
    );
    assert!(
        response
            .headers
            .get("access-control-allow-credentials")
            .is_none()
    );
}
