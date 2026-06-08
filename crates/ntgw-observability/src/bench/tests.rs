use crate::bench::{
    AccessLogBenchConfig, AccessLogFixture, TrafficStatsBenchConfig, TrafficStatsFixture,
    TrafficStatsTopologyMode,
};

#[test]
fn access_log_fixture_renders_and_writes_json_log() {
    let fixture = AccessLogFixture::build(AccessLogBenchConfig::default());

    let step = fixture.write_once().expect("access log write step");

    assert!(step.rendered_bytes > 32);
    assert!(step.file_bytes >= step.rendered_bytes as u64);
    assert_eq!(step.mode, "json");
    assert!(step.emitted);
    assert!(step.enabled);
    assert_eq!(step.sample_rate, 1.0);
}

#[test]
fn access_log_fixture_covers_disabled_fast_path() {
    let fixture = AccessLogFixture::build(AccessLogBenchConfig {
        enabled: false,
        ..AccessLogBenchConfig::default()
    });

    let step = fixture.write_once().expect("access log disabled step");

    assert!(!step.emitted);
    assert!(!step.enabled);
    assert_eq!(step.rendered_bytes, 0);
    assert_eq!(step.file_bytes, 0);
}

#[test]
fn access_log_fixture_covers_sampled_out_fast_path() {
    let fixture = AccessLogFixture::build(AccessLogBenchConfig {
        sample_rate: 0.0,
        ..AccessLogBenchConfig::default()
    });

    let step = fixture.write_once().expect("access log sampled-out step");

    assert!(!step.emitted);
    assert!(step.enabled);
    assert_eq!(step.sample_rate, 0.0);
    assert_eq!(step.rendered_bytes, 0);
    assert_eq!(step.file_bytes, 0);
}

#[test]
fn traffic_stats_fixture_observes_reused_topology() {
    let fixture = TrafficStatsFixture::build(
        TrafficStatsBenchConfig { shard_count: 4 },
        TrafficStatsTopologyMode::ReusedTopology,
    );

    fixture.observe_once();
    fixture.observe_once();
    let step = fixture.snapshot_step();

    assert_eq!(step.topology_mode, "reused_topology");
    assert_eq!(step.shard_count, 4);
    assert!(step.provided_topology);
    assert!(!step.has_backend_topology);
    assert_eq!(step.total_events, 2);
    assert_eq!(step.total_request_events, 2);
    assert_eq!(step.request_latency_histogram_count, 1);
}

#[test]
fn traffic_stats_fixture_observes_no_route_with_cached_topology() {
    let fixture = TrafficStatsFixture::build(
        TrafficStatsBenchConfig { shard_count: 4 },
        TrafficStatsTopologyMode::NoRoute,
    );

    fixture.observe_once();
    let step = fixture.snapshot_step();

    assert_eq!(step.topology_mode, "no_route");
    assert!(step.provided_topology);
    assert!(!step.has_backend_topology);
    assert_eq!(step.total_events, 1);
    assert_eq!(step.response_flag_count, 1);
    assert!(step.node_count >= 3);
    assert!(step.edge_count >= 2);
}

#[test]
fn traffic_stats_fixture_observes_backend_topology() {
    let fixture = TrafficStatsFixture::build(
        TrafficStatsBenchConfig { shard_count: 64 },
        TrafficStatsTopologyMode::BackendTopology,
    );

    fixture.observe_once();
    let step = fixture.snapshot_step();

    assert_eq!(step.topology_mode, "backend_topology");
    assert_eq!(step.shard_count, 64);
    assert!(step.provided_topology);
    assert!(step.has_backend_topology);
    assert_eq!(step.total_events, 1);
    assert!(step.node_count >= 5);
    assert!(step.edge_count >= 4);
}
