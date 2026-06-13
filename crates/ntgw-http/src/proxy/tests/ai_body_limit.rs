use super::super::filters::ai_request_body_limit_exceeded;

#[test]
fn ai_request_body_limit_allows_unlimited_when_disabled() {
    assert!(!ai_request_body_limit_exceeded(usize::MAX - 1, 1, 0));
}

#[test]
fn ai_request_body_limit_allows_body_at_limit() {
    assert!(!ai_request_body_limit_exceeded(900, 100, 1_000));
}

#[test]
fn ai_request_body_limit_rejects_body_over_limit() {
    assert!(ai_request_body_limit_exceeded(900, 101, 1_000));
}

#[test]
fn ai_request_body_limit_rejects_saturating_overflow() {
    assert!(ai_request_body_limit_exceeded(usize::MAX, 1, 1_000));
}
