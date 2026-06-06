use std::collections::BTreeSet;

use crate::report::{build_report, BenchConfig};

fn default_bench_config(iterations: u32) -> BenchConfig {
    BenchConfig {
        iterations,
        snapshot: ntgw_ir::bench::SnapshotBenchConfig::default(),
        tls_rotation: ntgw_http::runtime_bench::TlsRotationBenchConfig::default(),
        xds_apply: ntgw_xds::bench::ApplyBenchConfig::default(),
        request_meta: ntgw_http::bench::RequestMetaBuildBenchConfig::default(),
        filter_chain: ntgw_http::bench::FilterChainBenchConfig::default(),
        session_persistence: ntgw_http::bench::SessionBenchConfig::default(),
        access_log: ntgw_observability::bench::AccessLogBenchConfig::default(),
        traffic_stats: ntgw_observability::bench::TrafficStatsBenchConfig::default(),
        http_capacity: ntgw_http::runtime_bench::HttpCapacityMatrixBenchConfig::default(),
        stream: ntgw_stream::bench::StreamBenchConfig::default(),
    }
}

#[tokio::test(flavor = "current_thread")]
async fn report_tracks_formal_microbenchmark_scenarios() {
    let report = build_report(default_bench_config(1))
        .await
        .expect("formal microbenchmark report");

    let scenario_names = report
        .scenarios
        .iter()
        .map(|scenario| scenario.name.as_str())
        .collect::<BTreeSet<_>>();

    for required in [
        "http_route_selection",
        "grpc_route_selection",
        "stream_route_selection",
        "xds_snapshot_parse",
        "request_meta_header_heavy",
        "request_view_header_heavy",
        "request_fast_path_selection",
        "snapshot_read_rwlock",
        "snapshot_read_arc_swap",
        "runtime_index_rebuild_route_only",
        "runtime_index_rebuild_endpoint_only",
        "runtime_index_rebuild_secret_only",
        "header_filter_chain",
        "session_persistence",
        "access_log_disabled_path",
        "access_log_sampled_out_path",
        "access_log_write_path",
        "traffic_observe_reused_topology",
        "traffic_observe_no_route",
        "traffic_observe_backend_topology_4_shards",
        "traffic_observe_backend_topology_64_shards",
        "http_capacity_matrix",
        "stream_tcp_buffer_matrix",
        "stream_udp_dispatcher_distribution",
        "stream_udp_payload_copy",
    ] {
        assert!(
            scenario_names.contains(required),
            "missing required scenario {required}"
        );
    }
}

#[test]
fn request_fast_path_bench_report_contains_selection_details() {
    let report = crate::scenarios::run_request_fast_path_selection(
        4,
        ntgw_http::bench::RequestMetaBuildBenchConfig::default(),
    )
    .expect("fast path benchmark report");

    assert_eq!(report.name, "request_fast_path_selection");
    assert_eq!(report.iterations, 4);
    assert!(report.details["selected_backend"].as_str().is_some());
    assert!(report.details["route_name"].as_str().is_some());
}

#[test]
fn traffic_stats_bench_report_contains_topology_details() {
    let report = crate::scenarios::run_traffic_observe_backend_topology_4_shards(
        4,
        ntgw_observability::bench::TrafficStatsBenchConfig::default(),
    )
    .expect("traffic stats benchmark report");

    assert_eq!(report.name, "traffic_observe_backend_topology_4_shards");
    assert_eq!(report.iterations, 4);
    assert_eq!(report.details["topology_mode"], "backend_topology");
    assert_eq!(report.details["shard_count"], 4);
    assert_eq!(report.details["provided_topology"], true);
    assert_eq!(report.details["has_backend_topology"], true);
    assert_eq!(report.details["total_events"], 4);
    assert!(report.details["node_count"].as_u64().unwrap_or_default() >= 4);
    assert!(report.details["edge_count"].as_u64().unwrap_or_default() >= 4);
}

#[test]
fn traffic_stats_no_route_bench_models_cached_topology() {
    let report = crate::scenarios::run_traffic_observe_no_route(
        4,
        ntgw_observability::bench::TrafficStatsBenchConfig::default(),
    )
    .expect("no-route traffic stats benchmark report");

    assert_eq!(report.name, "traffic_observe_no_route");
    assert_eq!(report.details["topology_mode"], "no_route");
    assert_eq!(report.details["provided_topology"], true);
    assert_eq!(report.details["has_backend_topology"], false);
    assert_eq!(report.details["total_events"], 4);
}

#[test]
fn http_capacity_matrix_report_contains_parallelism_rows() {
    let report = crate::scenarios::run_http_capacity_matrix(
        2,
        ntgw_http::runtime_bench::HttpCapacityMatrixBenchConfig::default(),
    )
    .expect("http capacity matrix report");

    assert_eq!(report.name, "http_capacity_matrix");
    assert_eq!(report.iterations, 2);
    assert_eq!(report.details["row_count"], 16);
    assert_eq!(report.details["default_rows"], 8);
    assert_eq!(report.details["tuned_rows"], 8);
    assert_eq!(report.details["min_parallelism"], 1);
    assert_eq!(report.details["max_parallelism"], 128);
    assert_eq!(
        report.details["evidence_type"],
        "capacity-derivation-matrix"
    );

    let rows = report.details["rows"].as_array().expect("capacity rows");
    let one_cpu_default = rows
        .iter()
        .find(|row| row["profile"] == "default" && row["parallelism"] == 1)
        .expect("1-cpu default row");
    assert_eq!(one_cpu_default["effective_worker_threads"], 2);
    assert_eq!(one_cpu_default["effective_accept_concurrency"], 1);

    let tuned = rows
        .iter()
        .find(|row| row["profile"] == "tuned" && row["parallelism"] == 128)
        .expect("tuned high parallelism row");
    assert_eq!(tuned["effective_worker_threads"], 4);
    assert_eq!(tuned["effective_accept_concurrency"], 3);
    assert_eq!(tuned["effective_upstream_keepalive_pool_size"], 4096);
    assert_eq!(tuned["effective_reuse_port"], false);
}

#[test]
fn stream_bench_reports_tcp_udp_tuning_details() {
    let config = ntgw_stream::bench::StreamBenchConfig::default();

    let tcp = crate::scenarios::run_stream_tcp_buffer_matrix(2, config)
        .expect("tcp buffer matrix report");
    assert_eq!(tcp.name, "stream_tcp_buffer_matrix");
    assert_eq!(tcp.details["default_bytes"], 16 * 1024);
    assert_eq!(tcp.details["min_bytes"], 4 * 1024);
    assert_eq!(tcp.details["max_bytes"], 256 * 1024);
    assert_eq!(tcp.details["row_count"], 7);
    assert_eq!(tcp.details["evidence_type"], "tcp-buffer-tuning-matrix");

    let udp_distribution = crate::scenarios::run_stream_udp_dispatcher_distribution(1, config)
        .expect("udp distribution report");
    assert_eq!(udp_distribution.name, "stream_udp_dispatcher_distribution");
    assert_eq!(udp_distribution.details["clients"], 4096);
    assert_eq!(udp_distribution.details["dispatcher_workers"], 16);
    assert_eq!(udp_distribution.details["session_shards"], 16);
    assert_eq!(udp_distribution.details["non_empty_dispatcher_workers"], 16);
    assert_eq!(udp_distribution.details["non_empty_session_shards"], 16);

    let udp_copy =
        crate::scenarios::run_stream_udp_payload_copy(3, config).expect("udp copy report");
    assert_eq!(udp_copy.name, "stream_udp_payload_copy");
    assert_eq!(udp_copy.iterations, 3);
    assert_eq!(udp_copy.details["payload_bytes"], 1200);
    assert_eq!(udp_copy.details["copied_bytes"], 1200);
    assert_eq!(
        udp_copy.details["evidence_type"],
        "udp-payload-copy-hot-path"
    );
}

#[tokio::test(flavor = "current_thread")]
async fn report_records_selected_allocator() {
    let report = build_report(default_bench_config(1))
        .await
        .expect("report should build");

    let expected = if cfg!(feature = "allocator-mimalloc") {
        "mimalloc"
    } else if cfg!(feature = "allocator-jemalloc") {
        "jemalloc"
    } else {
        "system"
    };

    assert_eq!(report.allocator, expected);
}

#[tokio::test(flavor = "current_thread")]
async fn report_compares_access_log_fast_paths_against_full_write() {
    let report = build_report(default_bench_config(1))
        .await
        .expect("report should build");

    let comparison_names = report
        .comparisons
        .iter()
        .map(|comparison| comparison.name.as_str())
        .collect::<BTreeSet<_>>();

    assert!(comparison_names.contains("access_log_disabled_vs_full_write"));
    assert!(comparison_names.contains("access_log_sampled_out_vs_full_write"));
}

#[tokio::test(flavor = "current_thread")]
async fn report_compares_fast_path_selection_against_request_meta() {
    let report = build_report(default_bench_config(1))
        .await
        .expect("report should build");

    let comparison = report
        .comparisons
        .iter()
        .find(|comparison| comparison.name == "request_fast_path_vs_meta_header_heavy")
        .expect("fast path comparison");

    assert_eq!(comparison.baseline, "request_meta_header_heavy");
    assert_eq!(comparison.current, "request_fast_path_selection");
    assert_eq!(
        comparison.details["production_path_changed"].as_bool(),
        Some(true)
    );
}

#[tokio::test(flavor = "current_thread")]
async fn report_records_allocation_pressure_for_system_allocator() {
    let report = build_report(default_bench_config(1))
        .await
        .expect("report should build");

    let serialized = serde_json::to_value(&report).expect("report should serialize");
    let scenario = serialized["scenarios"]
        .as_array()
        .expect("scenario list")
        .iter()
        .find(|scenario| scenario["name"] == "request_meta_header_heavy")
        .expect("request_meta_header_heavy scenario");
    let http_route_selection = serialized["scenarios"]
        .as_array()
        .expect("scenario list")
        .iter()
        .find(|scenario| scenario["name"] == "http_route_selection")
        .expect("http_route_selection scenario");
    let grpc_route_selection = serialized["scenarios"]
        .as_array()
        .expect("scenario list")
        .iter()
        .find(|scenario| scenario["name"] == "grpc_route_selection")
        .expect("grpc_route_selection scenario");
    let route_only_rebuild = serialized["scenarios"]
        .as_array()
        .expect("scenario list")
        .iter()
        .find(|scenario| scenario["name"] == "runtime_index_rebuild_route_only")
        .expect("route-only runtime index rebuild scenario");
    let view_scenario = serialized["scenarios"]
        .as_array()
        .expect("scenario list")
        .iter()
        .find(|scenario| scenario["name"] == "request_view_header_heavy")
        .expect("request_view_header_heavy scenario");

    assert_eq!(
        http_route_selection["details"]["listener_candidate_model"],
        "single-pass-best-host-score"
    );
    assert_eq!(
        grpc_route_selection["details"]["listener_candidate_model"],
        "single-pass-best-host-score"
    );
    assert_eq!(
        route_only_rebuild["details"]["index_strategy"],
        "full-rebuild-baseline"
    );
    assert_eq!(route_only_rebuild["details"]["mutation"], "route-only");
    assert_eq!(view_scenario["details"]["request_id"], "bench-request-id");
    assert_eq!(view_scenario["details"]["content_length"], 1234);

    let comparison = serialized["comparisons"]
        .as_array()
        .expect("comparison list")
        .iter()
        .find(|comparison| comparison["name"] == "request_view_vs_meta_header_heavy")
        .expect("request view comparison");
    assert_eq!(comparison["baseline"], "request_meta_header_heavy");
    assert_eq!(comparison["current"], "request_view_header_heavy");
    assert!(comparison["timing_delta"]["p99_ms"].is_number());
    assert!(comparison["resource_delta"].is_object());

    let snapshot_read_comparison = serialized["comparisons"]
        .as_array()
        .expect("comparison list")
        .iter()
        .find(|comparison| comparison["name"] == "arc_swap_vs_rwlock_snapshot_read")
        .expect("snapshot read comparison");
    assert_eq!(snapshot_read_comparison["baseline"], "snapshot_read_rwlock");
    assert_eq!(
        snapshot_read_comparison["current"],
        "snapshot_read_arc_swap"
    );
    assert!(snapshot_read_comparison["timing_delta"]["p99_ms"].is_number());

    let delta = &scenario["resource_delta"];
    if report.allocator == "system" {
        assert!(
            delta["allocations"].as_i64().unwrap_or_default() > 0,
            "system allocator reports allocation count"
        );
        assert!(
            delta["bytes_allocated"].as_i64().unwrap_or_default() > 0,
            "system allocator reports allocated bytes"
        );
    } else {
        assert!(
            delta
                .get("allocations")
                .is_none_or(serde_json::Value::is_null),
            "custom allocator builds omit system allocation counters"
        );
    }
}
