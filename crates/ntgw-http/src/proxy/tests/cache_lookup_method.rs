use super::super::filters::cache_lookup_method_allowed;

#[test]
fn cache_lookup_method_allows_get_and_head_case_insensitively() {
    assert!(cache_lookup_method_allowed("GET"));
    assert!(cache_lookup_method_allowed("get"));
    assert!(cache_lookup_method_allowed("Head"));
    assert!(cache_lookup_method_allowed("HEAD"));
}

#[test]
fn cache_lookup_method_rejects_non_cacheable_methods() {
    assert!(!cache_lookup_method_allowed("POST"));
    assert!(!cache_lookup_method_allowed("PUT"));
    assert!(!cache_lookup_method_allowed(""));
}
