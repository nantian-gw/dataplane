use crate::bench::{
    FilterChainBenchConfig, FilterChainFixture, RequestMetaBuildBenchConfig,
    RequestMetaBuildFixture, RequestViewBuildBenchConfig, RequestViewBuildFixture,
    SessionBenchConfig, SessionBenchFixture,
};

#[test]
fn filter_chain_fixture_applies_request_and_response_headers() {
    let fixture = FilterChainFixture::build(FilterChainBenchConfig {
        request_filters: 2,
        response_filters: 2,
        header_ops_per_filter: 2,
    });

    let step = fixture.apply().expect("filter chain step");

    assert!(step.request_header_count >= 4);
    assert!(step.response_header_count >= 4);
    assert_eq!(step.request_marker, "set-0-0");
    assert_eq!(step.response_marker, "set-0-0");
}

#[test]
fn session_fixture_round_trips_cookie_transport() {
    let fixture =
        SessionBenchFixture::build(SessionBenchConfig::default()).expect("session bench fixture");

    let step = fixture
        .encode_decode_cycle()
        .expect("session encode decode cycle");

    assert_eq!(step.backend_name, "default/bench-backend:8080");
    assert_eq!(step.endpoint_address, "10.20.0.42");
    assert_eq!(step.endpoint_port, 8080);
    assert!(step.token_len > 32);
}

#[test]
fn request_meta_fixture_materializes_header_heavy_request() {
    let fixture = RequestMetaBuildFixture::build(RequestMetaBuildBenchConfig {
        header_count: 8,
        values_per_header: 3,
        query_params: 5,
        header_value_bytes: 16,
    })
    .expect("request meta bench fixture");

    let step = fixture.materialize().expect("request meta step");

    assert_eq!(step.path, "/bench/header-heavy");
    assert_eq!(step.query_param_count, 5);
    assert_eq!(step.header_name_count, 0, "headers are lazy in materialize()");
    assert_eq!(step.header_value_count, 0);
    assert_eq!(step.header_value_bytes, 0);
    assert_eq!(step.request_id, "");
    assert_eq!(step.content_length, 0);
}

#[test]
fn request_view_fixture_captures_header_heavy_request_without_materializing_headers() {
    let fixture = RequestViewBuildFixture::build(RequestViewBuildBenchConfig {
        header_count: 8,
        values_per_header: 3,
        query_params: 5,
        header_value_bytes: 16,
    })
    .expect("request view bench fixture");

    let step = fixture.capture().expect("request view step");

    assert_eq!(step.path, "/bench/header-heavy");
    assert_eq!(step.query_param_count, 5);
    assert_eq!(step.header_name_count, 0, "headers are lazy in materialize()");
    assert_eq!(step.header_value_count, 0);
    assert_eq!(step.header_value_bytes, 0);
    assert_eq!(step.request_id, "bench-request-id");
    assert_eq!(step.content_length, 1234);
    assert_eq!(step.client_ip, "192.0.2.20");
}
