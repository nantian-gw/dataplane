#[test]
fn apply_header_modifier_updates_request_headers() {
    let mut request = RequestHeader::build("GET", b"/", None).expect("request");
    request
        .insert_header("x-remove", "gone")
        .expect("insert remove header");
    request
        .insert_header("x-set", "old")
        .expect("insert set header");

    super::super::apply_header_modifier(
        &mut request,
        &HeaderModifier {
            set: vec![HeaderOperation {
                name: "x-set".to_string(),
                value: "new".to_string(),
            }],
            add: vec![HeaderOperation {
                name: "x-add".to_string(),
                value: "blue".to_string(),
            }],
            remove: vec!["x-remove".to_string()],
        },
    )
    .expect("apply request modifier");

    assert!(request.headers.get("x-remove").is_none());
    assert_eq!(
        request
            .headers
            .get("x-set")
            .and_then(|value| value.to_str().ok()),
        Some("new")
    );
    assert_eq!(
        request
            .headers
            .get("x-add")
            .and_then(|value| value.to_str().ok()),
        Some("blue")
    );
}
