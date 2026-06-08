use super::*;

#[tokio::test]
async fn http_route_admission_reports_route_scope_enabled_only_when_configured() {
    let disabled =
        HttpAdmissionController::new(HttpAdmissionOptions::default(), OverloadStats::shared());
    assert!(!disabled.route_scope_enabled());

    let enabled = HttpAdmissionController::new(
        HttpAdmissionOptions {
            route_inflight_limit: 1,
            ..HttpAdmissionOptions::default()
        },
        OverloadStats::shared(),
    );
    assert!(enabled.route_scope_enabled());
}

#[tokio::test]
async fn http_route_admission_tracks_current_usage_and_rejections() {
    let stats = OverloadStats::shared();
    let controller = HttpAdmissionController::new(
        HttpAdmissionOptions {
            route_inflight_limit: 1,
            ..HttpAdmissionOptions::default()
        },
        stats.clone(),
    );

    let permit = controller
        .try_acquire("default/gw/http", "Http/default/route")
        .expect("first route admission should succeed");

    let rejection = controller
        .try_acquire("default/gw/http", "Http/default/route")
        .expect_err("second route admission should fail");
    assert_eq!(rejection.scope_label(), "route");

    let snapshot = stats.snapshot();
    assert_eq!(snapshot.http_rejected_total, 1);
    assert_eq!(snapshot.http_rejected_route_total, 1);
    assert_eq!(
        snapshot
            .http_route_inflight_current
            .get("Http/default/route")
            .copied(),
        Some(1)
    );

    drop(permit);

    let snapshot = stats.snapshot();
    assert_eq!(
        snapshot
            .http_route_inflight_current
            .get("Http/default/route"),
        None
    );
}

#[tokio::test]
async fn http_keyed_admission_normalizes_listener_and_route_names() {
    let stats = OverloadStats::shared();
    let controller = HttpAdmissionController::new(
        HttpAdmissionOptions {
            listener_inflight_limit: 1,
            route_inflight_limit: 1,
            ..HttpAdmissionOptions::default()
        },
        stats.clone(),
    );

    let permit = controller
        .try_acquire(" default/gw/http ", " Http/default/route ")
        .expect("first keyed admission should succeed");

    let rejection = controller
        .try_acquire("default/gw/http", "Http/default/route")
        .expect_err("second keyed admission should fail on the same normalized listener");
    assert_eq!(rejection.scope_label(), "listener");

    let snapshot = stats.snapshot();
    assert_eq!(
        snapshot
            .http_listener_inflight_current
            .get("default/gw/http")
            .copied(),
        Some(1)
    );
    assert_eq!(
        snapshot
            .http_listener_inflight_current
            .get(" default/gw/http "),
        None
    );
    assert_eq!(
        snapshot
            .http_route_inflight_current
            .get("Http/default/route")
            .copied(),
        Some(1)
    );
    assert_eq!(
        snapshot
            .http_route_inflight_current
            .get(" Http/default/route "),
        None
    );

    drop(permit);
}
