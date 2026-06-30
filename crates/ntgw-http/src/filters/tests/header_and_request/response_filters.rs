#[test]
fn apply_response_filters_only_uses_response_header_modifiers() {
    let mut response = ResponseHeader::build(200, None).expect("response");

    super::super::apply_response_filters(
        &mut response,
        &[
            Filter {
                filter_type: "RequestHeaderModifier".to_string(),
                header_modifier: Some(HeaderModifier {
                    set: vec![HeaderOperation {
                        name: "x-ignore".to_string().into(),
                        value: "no".to_string(),
                    }],
                    ..HeaderModifier::default()
                }),
                ..Filter::default()
            },
            Filter {
                filter_type: "ResponseHeaderModifier".to_string(),
                header_modifier: Some(HeaderModifier {
                    set: vec![HeaderOperation {
                        name: "x-response".to_string().into(),
                        value: "ok".to_string(),
                    }],
                    ..HeaderModifier::default()
                }),
                ..Filter::default()
            },
        ],
        None,
        None,
    )
    .expect("apply response filters");

    assert!(response.headers.get("x-ignore").is_none());
    assert_eq!(
        response
            .headers
            .get("x-response")
            .and_then(|value| value.to_str().ok()),
        Some("ok")
    );
}
