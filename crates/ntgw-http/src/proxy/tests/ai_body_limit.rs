use super::super::filters::{ai_gateway_should_process_path, ai_request_body_limit_exceeded};

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

#[test]
fn ai_gateway_path_gate_only_matches_ai_provider_endpoints() {
    assert!(ai_gateway_should_process_path("/v1/chat/completions"));
    assert!(ai_gateway_should_process_path("/v1/messages"));
    assert!(ai_gateway_should_process_path("/api/chat"));
    assert!(!ai_gateway_should_process_path("/healthz"));
    assert!(!ai_gateway_should_process_path("/orders"));
}
