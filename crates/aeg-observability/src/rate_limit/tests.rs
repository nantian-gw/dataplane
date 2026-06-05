use std::{thread::sleep, time::Duration};

use super::{HttpRateLimitController, HttpRateLimitOptions};

#[test]
fn route_rate_limit_reports_route_scope_enabled_only_when_configured() {
    let disabled = HttpRateLimitController::new(HttpRateLimitOptions::default());
    assert!(!disabled.route_scope_enabled());

    let enabled = HttpRateLimitController::new(HttpRateLimitOptions {
        route_requests_per_second: 1,
        route_burst: 1,
        ..HttpRateLimitOptions::default()
    });
    assert!(enabled.route_scope_enabled());
}

#[test]
fn route_rejection_refunds_global_token_and_records_scope() {
    let controller = HttpRateLimitController::new(HttpRateLimitOptions {
        global_requests_per_second: 1,
        global_burst: 2,
        route_requests_per_second: 1,
        route_burst: 1,
        ..HttpRateLimitOptions::default()
    });

    controller
        .try_acquire("web", "Http/default/shared")
        .expect("first request should pass");
    controller.observe_allow();
    let first = controller.snapshot();
    assert_eq!(first.global.available_tokens, 1);
    assert_eq!(
        first
            .route
            .available_tokens_by_name
            .get("Http/default/shared")
            .copied(),
        Some(0)
    );

    let rejection = controller
        .try_acquire("web", "Http/default/shared")
        .expect_err("second request should hit the route limit");
    assert_eq!(rejection.scope_label(), "route");

    let snapshot = controller.snapshot();
    assert_eq!(snapshot.allowed_total, 1);
    assert_eq!(snapshot.rejected_total, 1);
    assert_eq!(snapshot.rejected_route_total, 1);
    assert_eq!(snapshot.global.available_tokens, 1);
    assert_eq!(
        snapshot
            .rejected_route_by_name
            .get("Http/default/shared")
            .copied(),
        Some(1)
    );
}

#[test]
fn listener_bucket_refills_after_elapsed_time() {
    let controller = HttpRateLimitController::new(HttpRateLimitOptions {
        listener_requests_per_second: 1,
        listener_burst: 1,
        ..HttpRateLimitOptions::default()
    });

    controller
        .try_acquire_listener("web")
        .expect("first request should pass");
    controller.observe_allow();
    assert!(controller.try_acquire_listener("web").is_err());

    sleep(Duration::from_millis(1_100));

    controller
        .try_acquire_listener("web")
        .expect("bucket should refill after one second");
    controller.observe_allow();

    let snapshot = controller.snapshot();
    assert_eq!(snapshot.allowed_total, 2);
    assert_eq!(snapshot.rejected_total, 1);
    assert_eq!(snapshot.rejected_listener_total, 1);
    assert_eq!(
        snapshot.rejected_listener_by_name.get("web").copied(),
        Some(1)
    );
    assert_eq!(
        snapshot.listener.requests_per_second, 1,
        "snapshot should preserve configured refill rate"
    );
}

#[test]
fn listener_rejection_refunds_global_token_and_records_scope() {
    let controller = HttpRateLimitController::new(HttpRateLimitOptions {
        global_requests_per_second: 1,
        global_burst: 2,
        listener_requests_per_second: 1,
        listener_burst: 1,
        ..HttpRateLimitOptions::default()
    });

    controller
        .try_acquire_listener("web")
        .expect("first request should pass");
    controller.observe_allow();

    let first = controller.snapshot();
    assert_eq!(first.global.available_tokens, 1);
    assert_eq!(
        first.listener.available_tokens_by_name.get("web").copied(),
        Some(0)
    );

    let rejection = controller
        .try_acquire_listener("web")
        .expect_err("second request should hit the listener limit");
    assert_eq!(rejection.scope_label(), "listener");

    let snapshot = controller.snapshot();
    assert_eq!(snapshot.allowed_total, 1);
    assert_eq!(snapshot.rejected_total, 1);
    assert_eq!(snapshot.rejected_listener_total, 1);
    assert_eq!(snapshot.global.available_tokens, 1);
    assert_eq!(
        snapshot.rejected_listener_by_name.get("web").copied(),
        Some(1)
    );
}
