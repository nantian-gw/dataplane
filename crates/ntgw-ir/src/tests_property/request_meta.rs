proptest! {
    #[test]
    fn request_meta_parses_query_pairs_and_uppercases_method(
        path_segment in path_segment_strategy(),
        raw_method in method_strategy(),
        query_pairs in prop_vec((mixed_case_token_strategy(), query_value_strategy()), 0..6),
    ) {
        let mut serializer = form_urlencoded::Serializer::new(String::new());
        let mut expected = std::collections::BTreeMap::<String, Vec<String>>::new();
        for (name, value) in &query_pairs {
            serializer.append_pair(name, value);
            expected
                .entry(name.to_ascii_lowercase())
                .or_default()
                .push(value.clone());
        }

        let query = serializer.finish();
        let raw = if query.is_empty() {
            format!("/{path_segment}")
        } else {
            format!("/{path_segment}?{query}")
        };
        let request = RequestMeta::new(
            Some("example.internal:8443".to_string()),
            raw.as_str(),
            raw_method.as_str(),
            std::collections::BTreeMap::new(),
        );

        prop_assert_eq!(request.path, format!("/{path_segment}"));
        prop_assert_eq!(request.port, 8443);
        prop_assert_eq!(request.method, raw_method.to_ascii_uppercase());
        prop_assert_eq!(request.query_params, expected);
    }
}
