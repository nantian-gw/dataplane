#[test]
fn render_metrics_exposes_retry_budget_state() {
    let state = test_state(None);

    let metrics = render_metrics(&state);

    assert!(metrics.contains("nantian_gateway_dataplane_http_retry_budget_enabled 1"));
    assert!(metrics.contains("nantian_gateway_dataplane_http_retry_budget_ratio_percent 20"));
    assert!(metrics.contains("nantian_gateway_dataplane_http_retry_budget_burst 16"));
    assert!(metrics.contains("nantian_gateway_dataplane_http_retry_budget_available_tokens 16"));
    assert!(
        metrics.contains("nantian_gateway_dataplane_http_retry_budget_available_milli_tokens 16000")
    );
    assert!(metrics.contains(
        "nantian_gateway_dataplane_http_retry_budget_retryable_requests_observed_total 0"
    ));
    assert!(metrics.contains("nantian_gateway_dataplane_http_retry_budget_allowed_total 0"));
    assert!(metrics.contains("nantian_gateway_dataplane_http_retry_budget_rejected_total 0"));
}
