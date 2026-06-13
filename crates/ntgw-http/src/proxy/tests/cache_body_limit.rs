use super::super::cache_response_body_limit_exceeded;

#[test]
fn cache_response_body_limit_allows_unlimited_when_disabled() {
    assert!(!cache_response_body_limit_exceeded(usize::MAX - 1, 1, 0));
}

#[test]
fn cache_response_body_limit_allows_body_at_limit() {
    assert!(!cache_response_body_limit_exceeded(900, 100, 1_000));
}

#[test]
fn cache_response_body_limit_rejects_body_over_limit() {
    assert!(cache_response_body_limit_exceeded(900, 101, 1_000));
}

#[test]
fn cache_response_body_limit_rejects_saturating_overflow() {
    assert!(cache_response_body_limit_exceeded(usize::MAX, 1, 1_000));
}
