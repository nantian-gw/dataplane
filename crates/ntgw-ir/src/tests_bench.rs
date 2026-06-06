#[test]
fn snapshot_switch_fixture_builds_large_reusable_reload_input() {
    let fixture = crate::bench::build_snapshot_switch_fixture(crate::bench::SnapshotBenchConfig {
        listeners: 4,
        routes_per_listener: 3,
        backends_per_route: 2,
        endpoints_per_backend: 2,
    });

    assert_eq!(fixture.current.listeners.len(), 4);
    assert_eq!(fixture.current.http_routes.len(), 12);
    assert_eq!(fixture.current.backends.len(), 24);
    assert_eq!(fixture.next.http_routes.len(), 12);
    assert!(fixture.current.select_backend(&fixture.probe_request).is_some());

    let mut next = fixture.next.clone();
    next.inherit_runtime_state_from(&fixture.current);

    assert!(next.select_backend(&fixture.probe_request).is_some());
    assert_eq!(
        next.backend_protocol(fixture.expected_backend_name.as_str()),
        Some("HTTP")
    );
}

#[test]
fn route_selection_fixture_covers_http_grpc_and_stream_paths() {
    let config = crate::bench::SnapshotBenchConfig {
        listeners: 3,
        routes_per_listener: 2,
        backends_per_route: 2,
        endpoints_per_backend: 2,
    };

    let http_fixture = crate::bench::build_route_selection_fixture(config);

    let http = http_fixture
        .snapshot
        .select_backend(&http_fixture.http_request)
        .expect("http backend");
    assert_eq!(http.backend_name, http_fixture.expected_http_backend_name);

    let grpc_fixture = crate::bench::build_route_selection_fixture(config);

    let grpc = grpc_fixture
        .snapshot
        .select_grpc_backend(&grpc_fixture.grpc_request)
        .expect("grpc backend");
    assert_eq!(grpc.backend_name, grpc_fixture.expected_grpc_backend_name);

    let stream_fixture = crate::bench::build_route_selection_fixture(config);

    let stream = stream_fixture
        .snapshot
        .select_stream_backend(
            stream_fixture.stream_listener_name.as_str(),
            Some(stream_fixture.stream_server_name.as_str()),
        )
        .expect("stream backend");
    assert_eq!(stream.backend_name, stream_fixture.expected_stream_backend_name);
}

#[test]
fn proto_snapshot_fixture_builds_large_decode_input() {
    let fixture = crate::bench::build_proto_snapshot_fixture(crate::bench::SnapshotBenchConfig {
        listeners: 2,
        routes_per_listener: 3,
        backends_per_route: 2,
        endpoints_per_backend: 2,
    });

    let snapshot = crate::Snapshot::from(fixture.snapshot.clone());

    assert_eq!(snapshot.listeners.len(), fixture.expected_listener_count);
    assert_eq!(snapshot.http_routes.len(), fixture.expected_http_routes);
    assert_eq!(snapshot.grpc_routes.len(), fixture.expected_grpc_routes);
    assert_eq!(snapshot.stream_routes.len(), fixture.expected_stream_routes);
    assert_eq!(snapshot.backends.len(), fixture.expected_backends);
}
