#[test]
fn apply_request_filters_rewrites_path_and_host_in_order() {
    let mut request = RequestHeader::build("GET", b"/users?id=1", None).expect("request");
    request
        .insert_header("host", "api.example.com")
        .expect("insert host");

    super::super::apply_request_filters(
        &mut request,
        &[
            Filter {
                filter_type: "RequestHeaderModifier".to_string(),
                header_modifier: Some(HeaderModifier {
                    set: vec![HeaderOperation {
                        name: "host".to_string().into(),
                        value: "pre.example.com".to_string(),
                    }],
                    ..HeaderModifier::default()
                }),
                ..Filter::default()
            },
            Filter {
                filter_type: "URLRewrite".to_string(),
                url_rewrite: Some(UrlRewriteFilter {
                    hostname: "backend.internal".to_string(),
                    path: Some(PathModifier {
                        modifier_type: "ReplacePrefixMatch".to_string(),
                        replace_prefix_match: "/api".to_string(),
                        ..PathModifier::default()
                    }),
                }),
                ..Filter::default()
            },
        ],
        Some(&MatchedHttpPath {
            path: "/users".to_string(),
            path_type: "PathPrefix".to_string(),
        }),
    )
    .expect("apply request filters");

    assert_eq!(
        request
            .headers
            .get("host")
            .and_then(|value| value.to_str().ok()),
        Some("backend.internal")
    );
    assert_eq!(request.uri.path(), "/api");
    assert_eq!(request.uri.query(), Some("id=1"));
}
