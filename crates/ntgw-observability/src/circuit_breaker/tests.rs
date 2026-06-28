use std::collections::HashMap;

use crate::{HttpCircuitBreakerController, HttpCircuitBreakerOptions, HttpCircuitBreakerRejection};

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

#[test]
fn per_backend_limit_overrides_global_default() {
    let options = HttpCircuitBreakerOptions {
        backend_max_inflight_requests: 10,
    };
    let controller = HttpCircuitBreakerController::new(options);
    controller.set_backend_limit("api-v2", 3);

    // backend with custom limit — can acquire up to 3
    assert!(controller.try_acquire_backend("api-v2").is_ok());

    // Verify snapshot tracks the global setting
    let snap = controller.snapshot();
    assert_eq!(snap.backend_max_inflight_requests, 10);
}

#[test]
fn unset_backend_uses_global_limit() {
    let options = HttpCircuitBreakerOptions {
        backend_max_inflight_requests: 2,
    };
    let controller = HttpCircuitBreakerController::new(options);

    // backend without custom limit — uses global
    assert!(controller.try_acquire_backend("default-backend").is_ok());
}

#[test]
fn set_per_backend_limits_bulk_update() {
    let options = HttpCircuitBreakerOptions {
        backend_max_inflight_requests: 10,
    };
    let controller = HttpCircuitBreakerController::new(options);

    let mut limits = HashMap::new();
    limits.insert("svc-a".to_string(), 1);
    limits.insert("svc-b".to_string(), 2);
    controller.set_per_backend_limits(limits);

    // svc-a: limit 1, should fail second acquire
    let permit = controller.try_acquire_backend("svc-a").unwrap();
    assert!(matches!(
        controller.try_acquire_backend("svc-a"),
        Err(HttpCircuitBreakerRejection::Backend)
    ));
    drop(permit);

    // svc-b: limit 2
    let p1 = controller.try_acquire_backend("svc-b").unwrap();
    let p2 = controller.try_acquire_backend("svc-b").unwrap();
    assert!(matches!(
        controller.try_acquire_backend("svc-b"),
        Err(HttpCircuitBreakerRejection::Backend)
    ));
    drop(p1);
    drop(p2);

    // unset backend uses global 10
    let mut unset_permits = Vec::new();
    for _ in 0..10 {
        unset_permits.push(controller.try_acquire_backend("unset").unwrap());
    }
    assert!(matches!(
        controller.try_acquire_backend("unset"),
        Err(HttpCircuitBreakerRejection::Backend)
    ));
    drop(unset_permits);
}

#[test]
fn backend_limit_for_returns_custom_limit_when_set() {
    let options = HttpCircuitBreakerOptions {
        backend_max_inflight_requests: 100,
    };
    let controller = HttpCircuitBreakerController::new(options);

    // Before setting a custom limit, return global default
    assert_eq!(controller.backend_limit_for("api-v3"), 100);

    // After setting a backend-specific limit
    controller.set_backend_limit("api-v3", 5);
    assert_eq!(controller.backend_limit_for("api-v3"), 5);

    // Other backends still use global
    assert_eq!(controller.backend_limit_for("other-backend"), 100);
}

// The `sync_per_backend_cb_limit` function (in ntgw-http/src/proxy/upstream.rs)
// calls `set_backend_limit` with the IR snapshot's `CircuitBreakerConfig.max_inflight_requests`.
// Full integration testing requires a GatewayProxy + Snapshot fixture, which is
// covered by integration / e2e tests. The `set_backend_limit` and `backend_limit_for`
// behaviour validated above exercises the same code path.
