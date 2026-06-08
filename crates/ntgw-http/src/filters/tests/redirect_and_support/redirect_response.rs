use super::*;

#[test]
fn request_redirect_filter_returns_first_redirect() {
    let filters = [
        Filter {
            filter_type: "RequestHeaderModifier".to_string(),
            header_modifier: Some(HeaderModifier::default()),
            ..Filter::default()
        },
        Filter {
            filter_type: "RequestRedirect".to_string(),
            request_redirect: Some(RequestRedirectFilter {
                scheme: "https".to_string(),
                ..RequestRedirectFilter::default()
            }),
            ..Filter::default()
        },
    ];
    let redirect = request_redirect_filter(&filters).expect("redirect");

    assert_eq!(redirect.scheme, "https");
}

#[test]
fn build_redirect_response_preserves_gateway_api_redirect_status_codes() {
    for code in [303_u16, 307, 308] {
        let response =
            build_redirect_response(code, "https://example.com/next").expect("redirect response");
        assert_eq!(response.status.as_u16(), code);
        assert_eq!(
            response
                .headers
                .get("location")
                .and_then(|value| value.to_str().ok()),
            Some("https://example.com/next")
        );
    }
}
