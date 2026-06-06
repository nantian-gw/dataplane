use crate::{HttpCircuitBreakerController, HttpCircuitBreakerOptions};

#[test]
fn backend_circuit_breaker_tracks_current_usage_and_rejections() {
    let controller = HttpCircuitBreakerController::new(HttpCircuitBreakerOptions {
        backend_max_inflight_requests: 1,
    });

    let permit = controller
        .try_acquire_backend("default/echo:8080")
        .expect("first request should pass");

    let rejection = controller
        .try_acquire_backend("default/echo:8080")
        .expect_err("second request should be rejected");
    assert_eq!(rejection.scope_label(), "backend");

    let snapshot = controller.snapshot();
    assert_eq!(
        snapshot
            .backend_inflight_current
            .get("default/echo:8080")
            .copied(),
        Some(1)
    );
    assert_eq!(snapshot.rejected_total, 1);
    assert_eq!(snapshot.rejected_backend_total, 1);
    assert_eq!(
        snapshot
            .rejected_backend_by_name
            .get("default/echo:8080")
            .copied(),
        Some(1)
    );

    drop(permit);

    let snapshot = controller.snapshot();
    assert!(
        snapshot.backend_inflight_current.is_empty(),
        "permit drop should release current usage"
    );
}
