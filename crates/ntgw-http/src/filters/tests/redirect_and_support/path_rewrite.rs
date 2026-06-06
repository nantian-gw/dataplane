use super::*;

#[test]
fn rewrite_path_and_query_handles_prefix_and_full_path() {
    let full = rewrite_path_and_query(
        "/users/42",
        Some("debug=true"),
        &PathModifier {
            modifier_type: "ReplaceFullPath".to_string(),
            replace_full_path: "/members/42".to_string(),
            ..PathModifier::default()
        },
        None,
    );
    let prefix = rewrite_path_and_query(
        "/users/42",
        Some("debug=true"),
        &PathModifier {
            modifier_type: "ReplacePrefixMatch".to_string(),
            replace_prefix_match: "/api".to_string(),
            ..PathModifier::default()
        },
        Some(&MatchedHttpPath {
            path: "/users".to_string(),
            path_type: "PathPrefix".to_string(),
        }),
    );

    assert_eq!(full, "/members/42?debug=true");
    assert_eq!(prefix, "/api/42?debug=true");
}
