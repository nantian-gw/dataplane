use super::super::{cache_response_body_limit_exceeded, response_body_filter_should_buffer};

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

#[test]
fn response_body_filter_buffers_for_cache_only() {
    assert!(response_body_filter_should_buffer(true, false));
}

#[test]
fn response_body_filter_buffers_for_active_ai_post_process_only() {
    assert!(response_body_filter_should_buffer(false, true));
}

#[test]
fn response_body_filter_does_not_buffer_when_ai_gateway_enabled_without_ai_context() {
    assert!(!response_body_filter_should_buffer(false, false));
}
