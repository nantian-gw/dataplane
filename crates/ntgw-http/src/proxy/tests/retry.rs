use ntgw_ir::RetryPolicy;
use ntgw_observability::{HttpCircuitBreakerController, HttpCircuitBreakerOptions};

use super::super::retry::{
    proxy_error_code, proxy_error_flag, proxy_error_flag_for, proxy_error_status, retry_limit,
    should_suppress_proxy_error_log,
};

#[test]
fn proxy_error_status_maps_timeout_errors_to_gateway_timeout() {
    assert_eq!(proxy_error_status("ReadTimedout"), 504);
    assert_eq!(proxy_error_status("WriteTimedout"), 504);
    assert_eq!(proxy_error_status("ConnectTimedout"), 504);
    assert_eq!(proxy_error_status("TLSHandshakeTimedout"), 504);
    assert_eq!(proxy_error_status("RequestTimeout"), 504);
}

#[test]
fn proxy_error_status_preserves_route_and_backend_mappings() {
    assert_eq!(proxy_error_status("NoRouteMatched"), 404);
    assert_eq!(proxy_error_status("NoHealthyBackend"), 503);
    assert_eq!(proxy_error_status("CircuitBreakerOpen"), 503);
    assert_eq!(proxy_error_status("UnsupportedRouteFilter"), 500);
    assert_eq!(proxy_error_status("InvalidBackendRefs"), 500);
}

#[test]
fn retry_limit_defaults_to_one_when_attempts_unspecified() {
    assert_eq!(
        retry_limit(&RetryPolicy {
            codes: vec![503],
            attempts: 0,
            backoff: None,
        }),
        1
    );
    assert_eq!(
        retry_limit(&RetryPolicy {
            codes: vec![503],
            attempts: 3,
            backoff: None,
        }),
        3
    );
}

#[test]
fn proxy_error_flag_maps_known_gateway_failure_classes() {
    assert_eq!(proxy_error_flag("NoRouteMatched"), "NR");
    assert_eq!(proxy_error_flag("NoHealthyBackend"), "UH");
    assert_eq!(proxy_error_flag("CircuitBreakerOpen"), "CB");
    assert_eq!(proxy_error_flag("InvalidBackendRefs"), "IB");
    assert_eq!(proxy_error_flag("ReadTimedout"), "UT");
    assert_eq!(proxy_error_flag("TLSHandshakeTimedout"), "UT");
    assert_eq!(proxy_error_flag("SomeOtherFailure"), "UF");
}

#[test]
fn proxy_error_flag_uses_connection_close_source() {
    let downstream = pingora::Error::new_down(pingora::ErrorType::ConnectionClosed);
    let upstream = pingora::Error::new_up(pingora::ErrorType::ConnectionClosed);
    let timeout = pingora::Error::new_up(pingora::ErrorType::ReadTimedout);

    assert_eq!(proxy_error_flag_for(&downstream), "DC");
    assert_eq!(proxy_error_flag_for(&upstream), "UC");
    assert_eq!(proxy_error_flag_for(&timeout), "UT");
}

#[test]
fn proxy_error_code_preserves_explicit_http_status() {
    let error = pingora::Error::new(pingora::ErrorType::HTTPStatus(502));
    assert_eq!(proxy_error_code(&error), 502);
}

#[test]
fn downstream_read_timeout_is_reported_as_request_timeout() {
    let error = pingora::Error::new_down(pingora::ErrorType::ReadTimedout);

    assert_eq!(proxy_error_code(&error), 408);
    assert_eq!(proxy_error_flag_for(&error), "IT");
}

#[test]
fn downstream_connection_closed_is_reported_as_client_closed() {
    let error = pingora::Error::new_down(pingora::ErrorType::ConnectionClosed);

    assert_eq!(proxy_error_code(&error), 499);
    assert!(should_suppress_proxy_error_log(&error, false));
}

#[test]
fn post_response_upstream_close_suppresses_proxy_error_log() {
    let upstream_closed = pingora::Error::new_up(pingora::ErrorType::ConnectionClosed);
    let upstream_timeout = pingora::Error::new_up(pingora::ErrorType::ReadTimedout);

    assert!(!should_suppress_proxy_error_log(&upstream_closed, false));
    assert!(should_suppress_proxy_error_log(&upstream_closed, true));
    assert!(!should_suppress_proxy_error_log(&upstream_timeout, true));
}

#[test]
fn sync_per_backend_cb_limit_contract_set_limit_and_acquire_works() {
    let opts = HttpCircuitBreakerOptions {
        backend_max_inflight_requests: 100,
    };
    let controller = HttpCircuitBreakerController::new(opts);

    controller.set_backend_limit("test-backend", 3);
    let p1 = controller.try_acquire_backend("test-backend").unwrap();
    let p2 = controller.try_acquire_backend("test-backend").unwrap();
    let p3 = controller.try_acquire_backend("test-backend").unwrap();

    assert!(
        controller.try_acquire_backend("test-backend").is_err(),
        "4th acquire must be rejected after limit of 3 is set"
    );

    drop(p1);
    drop(p2);
    drop(p3);

    let snap = controller.snapshot();
    assert_eq!(snap.backend_max_inflight_requests, 100);
    assert_eq!(snap.rejected_backend_total, 1);
}

#[test]
fn snapshot_with_circuit_breaker_syncs_per_backend_limit() {
    use ntgw_ir::{BackendCluster, CircuitBreakerConfig, Snapshot};

    // Create snapshot with a backend that has CB config
    let mut snapshot = Snapshot::default();
    snapshot.backends.push(BackendCluster {
        name: "limited-svc".to_string().into(),
        namespace: "default".to_string().into(),
        circuit_breaker: Some(CircuitBreakerConfig {
            max_inflight_requests: 3,
        }),
        ..Default::default()
    });
    snapshot.backends.push(BackendCluster {
        name: "unlimited-svc".to_string().into(),
        namespace: "default".to_string().into(),
        circuit_breaker: None,
        ..Default::default()
    });

    let controller = HttpCircuitBreakerController::new(HttpCircuitBreakerOptions {
        backend_max_inflight_requests: 100,
    });

    // Simulate what sync_per_backend_cb_limit does
    for backend in &snapshot.backends {
        if let Some(ref cb) = backend.circuit_breaker {
            if cb.max_inflight_requests > 0 {
                controller.set_backend_limit(&backend.name, cb.max_inflight_requests as usize);
            }
        }
    }

    // Limited backend: 3 permits should succeed, 4th should fail
    let p1 = controller.try_acquire_backend("limited-svc").unwrap();
    let p2 = controller.try_acquire_backend("limited-svc").unwrap();
    let p3 = controller.try_acquire_backend("limited-svc").unwrap();
    assert!(matches!(
        controller.try_acquire_backend("limited-svc"),
        Err(ntgw_observability::HttpCircuitBreakerRejection::Backend)
    ));
    drop(p1);
    drop(p2);
    drop(p3);

    // Unlimited backend: uses global limit of 100
    for _ in 0..100 {
        controller.try_acquire_backend("unlimited-svc").unwrap();
    }

    // Snapshot reflects correct max
    let snap = controller.snapshot();
    assert_eq!(snap.backend_max_inflight_requests, 100);
}

#[test]
fn proto_snapshot_with_cb_config_enforces_per_backend_limit() {
    use ntgw_ir::{BackendCluster, CircuitBreakerConfig, Snapshot};
    use ntgw_observability::{HttpCircuitBreakerController, HttpCircuitBreakerOptions};

    // Simulate the full pipeline: proto BackendCluster (from xDS snapshot)
    // → IR BackendCluster → HttpCircuitBreakerController → enforcement
    let snapshot = Snapshot {
        backends: vec![BackendCluster {
            name: "orders-svc".to_string().into(),
            namespace: "production".to_string().into(),
            circuit_breaker: Some(CircuitBreakerConfig {
                max_inflight_requests: 5,
            }),
            ..Default::default()
        }],
        ..Default::default()
    };

    let controller = HttpCircuitBreakerController::new(HttpCircuitBreakerOptions {
        backend_max_inflight_requests: 100,
    });

    // Simulate sync_per_backend_cb_limit: read CB config from IR → set on controller
    for backend in &snapshot.backends {
        if let Some(ref cb) = backend.circuit_breaker {
            if cb.max_inflight_requests > 0 {
                controller.set_backend_limit(&backend.name, cb.max_inflight_requests as usize);
            }
        }
    }

    // Verify per-backend limit (5) is enforced
    let permits: Vec<_> = (0..5)
        .map(|_| controller.try_acquire_backend("orders-svc").unwrap())
        .collect();
    assert!(matches!(
        controller.try_acquire_backend("orders-svc"),
        Err(ntgw_observability::HttpCircuitBreakerRejection::Backend)
    ));
    drop(permits);

    // Verify snapshot reflects the config
    let snap = controller.snapshot();
    assert_eq!(snap.backend_max_inflight_requests, 100);
}
