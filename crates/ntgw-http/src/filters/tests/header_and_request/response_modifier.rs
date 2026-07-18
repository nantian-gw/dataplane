#[test]
fn apply_header_modifier_updates_response_headers() {
    let mut response = ResponseHeader::build(200, None).expect("response");
    response
        .insert_header("server", "pingora")
        .expect("insert server header");

    super::super::apply_header_modifier(
        &mut response,
        &HeaderModifier {
            set: vec![HeaderOperation {
                name: "x-response".to_string(),
                value: "ok".to_string(),
            }],
            add: vec![],
            remove: vec!["server".to_string()],
        },
    )
    .expect("apply response modifier");

    assert!(response.headers.get("server").is_none());
    assert_eq!(
        response
            .headers
            .get("x-response")
            .and_then(|value| value.to_str().ok()),
        Some("ok")
    );
}
