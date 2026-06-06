use super::*;

#[test]
fn route_filters_and_detail_lookup_work() {
    let snapshot = fixture_snapshot();
    let query = RouteListQuery {
        kind: Some("tlsroute".to_string()),
        namespace: Some("default".to_string()),
        hostname: Some("secure.example.com".to_string()),
        ..RouteListQuery::default()
    };

    let routes = filter_routes(&snapshot, &query).expect("route filter");
    assert!(routes.http.is_empty());
    assert!(routes.grpc.is_empty());
    assert_eq!(routes.stream.len(), 1);
    assert_eq!(routes.stream[0].name, "passthrough");

    let detail = find_route(&snapshot, "route_kind_tls", "default", "passthrough")
        .expect("route detail lookup")
        .expect("route exists");
    assert_eq!(detail["kind"], "ROUTE_KIND_TLS");
}

#[test]
fn backend_filters_and_detail_lookup_work() {
    let snapshot = fixture_snapshot();
    let query = BackendListQuery {
        namespace: Some("default".to_string()),
        protocol: Some("http".to_string()),
        ..BackendListQuery::default()
    };

    let backends = filter_backends(&snapshot, &query).expect("backend filter");
    assert_eq!(backends.len(), 1);
    assert_eq!(backends[0].name, "api:80");

    let backend = find_backend(&snapshot, "default", "api:80").expect("backend detail");
    assert_eq!(backend.protocol, "HTTP");
}

#[test]
fn backend_filters_canonicalize_h2c_protocol() {
    let snapshot = fixture_snapshot();
    let query = BackendListQuery {
        protocol: Some("h2c".to_string()),
        ..BackendListQuery::default()
    };

    let backends = filter_backends(&snapshot, &query).expect("backend filter");
    assert_eq!(backends.len(), 1);
    assert_eq!(backends[0].name, "http2-clear:8080");
    assert_eq!(backends[0].protocol, "H2C");
}

#[test]
fn invalid_route_kind_is_rejected() {
    let snapshot = fixture_snapshot();
    let err = find_route(&snapshot, "not-a-kind", "default", "passthrough")
        .expect_err("invalid kind should fail");
    assert_eq!(err.status, StatusCode::BAD_REQUEST);
}
