proptest! {
    #[test]
    fn property_request_filter_chain_preserves_expected_header_transformations(
        set_value in filter_value_strategy(),
        add_value in filter_value_strategy(),
        rewrite_host in host_strategy(),
        rewrite_prefix in rewrite_prefix_strategy(),
    ) {
        let mut request = RequestHeader::build("GET", b"/users?id=42", None).expect("request");
        request
            .insert_header("host", "before.example.com")
            .expect("insert host");
        request
            .insert_header("x-remove", "gone")
            .expect("insert removable header");

        super::super::apply_request_filters(
            &mut request,
            &[
                Filter {
                    filter_type: "RequestHeaderModifier".to_string(),
                    header_modifier: Some(HeaderModifier {
                        set: vec![HeaderOperation {
                            name: "x-set".to_string(),
                            value: set_value.clone(),
                        }],
                        add: vec![HeaderOperation {
                            name: "x-add".to_string(),
                            value: add_value.clone(),
                        }],
                        remove: vec!["x-remove".to_string()],
                    }),
                    ..Filter::default()
                },
                Filter {
                    filter_type: "URLRewrite".to_string(),
                    url_rewrite: Some(UrlRewriteFilter {
                        hostname: rewrite_host.clone(),
                        path: Some(PathModifier {
                            modifier_type: "ReplacePrefixMatch".to_string(),
                            replace_prefix_match: rewrite_prefix.clone(),
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

        prop_assert!(request.headers.get("x-remove").is_none());
        prop_assert_eq!(
            request.headers.get("x-set").and_then(|value| value.to_str().ok()),
            Some(set_value.as_str())
        );
        prop_assert_eq!(
            request.headers.get("x-add").and_then(|value| value.to_str().ok()),
            Some(add_value.as_str())
        );
        prop_assert_eq!(
            request.headers.get("host").and_then(|value| value.to_str().ok()),
            Some(rewrite_host.as_str())
        );
        prop_assert_eq!(request.uri.path(), rewrite_prefix);
        prop_assert_eq!(request.uri.query(), Some("id=42"));
    }
}
