use std::{
    fs,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::Result;
use serde::Serialize;
use serde_json::json;

use crate::scenarios::{
    run_access_log_disabled_path, run_access_log_sampled_out_path, run_access_log_write_path,
    run_backend_tls_cache_key_construction, run_grpc_route_selection, run_header_filter_chain,
    run_high_frequency_apply, run_http_capacity_matrix, run_http_route_selection,
    run_large_snapshot_switch, run_last_good_fallback, run_request_fast_path_selection,
    run_request_meta_header_heavy, run_request_view_header_heavy,
    run_runtime_index_rebuild_endpoint_only, run_runtime_index_rebuild_route_only,
    run_runtime_index_rebuild_secret_only, run_session_persistence, run_snapshot_read_arc_swap,
    run_snapshot_read_rwlock, run_stream_pool_contention_hot_key,
    run_stream_pool_contention_spread, run_stream_route_selection, run_stream_tcp_buffer_matrix,
    run_stream_udp_dispatcher_distribution, run_stream_udp_payload_copy, run_tls_asset_rotation,
    run_traffic_observe_backend_topology_4_shards, run_traffic_observe_backend_topology_64_shards,
    run_traffic_observe_high_cardinality, run_traffic_observe_no_route,
    run_traffic_observe_reused_topology, run_wasm_hook_empty_invoke,
    run_wasm_hook_header_heavy_invoke, run_xds_snapshot_parse,
};

#[derive(Debug, Clone, Serialize)]
pub struct BenchConfig {
    pub iterations: u32,
    pub snapshot: ntgw_ir::bench::SnapshotBenchConfig,
    pub tls_rotation: ntgw_http::runtime_bench::TlsRotationBenchConfig,
    pub xds_apply: ntgw_xds::bench::ApplyBenchConfig,
    pub request_meta: ntgw_http::bench::RequestMetaBuildBenchConfig,
    pub filter_chain: ntgw_http::bench::FilterChainBenchConfig,
    pub session_persistence: ntgw_http::bench::SessionBenchConfig,
    pub access_log: ntgw_observability::bench::AccessLogBenchConfig,
    pub traffic_stats: ntgw_observability::bench::TrafficStatsBenchConfig,
    pub traffic_stats_cardinality: ntgw_observability::bench::TrafficStatsCardinalityBenchConfig,
    pub http_capacity: ntgw_http::runtime_bench::HttpCapacityMatrixBenchConfig,
    pub stream: ntgw_stream::bench::StreamBenchConfig,
    pub stream_pool_contention: ntgw_stream::bench::TcpPoolContentionBenchConfig,
}

#[derive(Debug, Clone, Serialize)]
pub struct BenchReport {
    pub(crate) captured_at_unix_seconds: u64,
    pub(crate) allocator: String,
    pub(crate) config: BenchConfig,
    pub(crate) scenarios: Vec<ScenarioReport>,
    pub(crate) comparisons: Vec<ScenarioComparisonReport>,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct ScenarioComparisonReport {
    pub(crate) name: String,
    pub(crate) baseline: String,
    pub(crate) current: String,
    pub(crate) timing_delta: TimingDelta,
    pub(crate) resource_delta: ResourceDelta,
    pub(crate) details: serde_json::Value,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct TimingDelta {
    pub(crate) average_ms: f64,
    pub(crate) p50_ms: f64,
    pub(crate) p95_ms: f64,
    pub(crate) p99_ms: f64,
    pub(crate) max_ms: f64,
    pub(crate) p99_reduction_ratio: Option<f64>,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct ScenarioReport {
    pub(crate) name: String,
    pub(crate) iterations: u32,
    pub(crate) timing: TimingSummary,
    pub(crate) resources_before: ResourceSample,
    pub(crate) resources_after: ResourceSample,
    pub(crate) resource_delta: ResourceDelta,
    pub(crate) details: serde_json::Value,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct TimingSummary {
    pub(crate) total_ms: f64,
    pub(crate) average_ms: f64,
    pub(crate) p50_ms: f64,
    pub(crate) p95_ms: f64,
    pub(crate) p99_ms: f64,
    pub(crate) max_ms: f64,
}

#[derive(Debug, Clone, Default, Serialize)]
pub(crate) struct ResourceSample {
    pub(crate) fd_count: Option<u64>,
    pub(crate) rss_kib: Option<u64>,
    pub(crate) threads: Option<u64>,
    pub(crate) cpu_user_ticks: Option<u64>,
    pub(crate) cpu_system_ticks: Option<u64>,
    pub(crate) allocations: Option<u64>,
    pub(crate) deallocations: Option<u64>,
    pub(crate) reallocations: Option<u64>,
    pub(crate) bytes_allocated: Option<u64>,
    pub(crate) bytes_deallocated: Option<u64>,
    pub(crate) bytes_reallocated: Option<i64>,
}

#[derive(Debug, Clone, Default, Serialize)]
pub(crate) struct ResourceDelta {
    pub(crate) fd_count: Option<i64>,
    pub(crate) rss_kib: Option<i64>,
    pub(crate) threads: Option<i64>,
    pub(crate) cpu_user_ticks: Option<i64>,
    pub(crate) cpu_system_ticks: Option<i64>,
    pub(crate) allocations: Option<i64>,
    pub(crate) deallocations: Option<i64>,
    pub(crate) reallocations: Option<i64>,
    pub(crate) bytes_allocated: Option<i64>,
    pub(crate) bytes_deallocated: Option<i64>,
    pub(crate) bytes_reallocated: Option<i64>,
}

pub async fn build_report(config: BenchConfig) -> Result<BenchReport> {
    let scenarios = vec![
        run_http_route_selection(config.iterations, config.snapshot)?,
        run_grpc_route_selection(config.iterations, config.snapshot)?,
        run_stream_route_selection(config.iterations, config.snapshot)?,
        run_xds_snapshot_parse(config.iterations, config.snapshot)?,
        run_large_snapshot_switch(config.iterations, config.snapshot)?,
        run_request_meta_header_heavy(config.iterations, config.request_meta)?,
        run_request_view_header_heavy(config.iterations, config.request_meta)?,
        run_request_fast_path_selection(config.iterations, config.request_meta)?,
        run_snapshot_read_rwlock(config.iterations, config.snapshot)?,
        run_snapshot_read_arc_swap(config.iterations, config.snapshot)?,
        run_runtime_index_rebuild_route_only(config.iterations, config.snapshot)?,
        run_runtime_index_rebuild_endpoint_only(config.iterations, config.snapshot)?,
        run_runtime_index_rebuild_secret_only(config.iterations, config.snapshot)?,
        run_header_filter_chain(config.iterations, config.filter_chain)?,
        run_session_persistence(config.iterations, config.session_persistence)?,
        run_backend_tls_cache_key_construction(config.iterations)?,
        run_access_log_disabled_path(config.iterations, config.access_log)?,
        run_access_log_sampled_out_path(config.iterations, config.access_log)?,
        run_access_log_write_path(config.iterations, config.access_log)?,
        run_traffic_observe_reused_topology(config.iterations, config.traffic_stats)?,
        run_traffic_observe_no_route(config.iterations, config.traffic_stats)?,
        run_traffic_observe_backend_topology_4_shards(config.iterations, config.traffic_stats)?,
        run_traffic_observe_backend_topology_64_shards(config.iterations, config.traffic_stats)?,
        run_traffic_observe_high_cardinality(config.iterations, config.traffic_stats_cardinality)?,
        run_http_capacity_matrix(config.iterations, config.http_capacity)?,
        run_stream_tcp_buffer_matrix(config.iterations, config.stream)?,
        run_stream_udp_dispatcher_distribution(config.iterations, config.stream)?,
        run_stream_udp_payload_copy(config.iterations, config.stream)?,
        run_stream_pool_contention_hot_key(config.iterations, config.stream_pool_contention)?,
        run_stream_pool_contention_spread(config.iterations, config.stream_pool_contention)?,
        run_wasm_hook_empty_invoke(config.iterations)?,
        run_wasm_hook_header_heavy_invoke(config.iterations)?,
        run_tls_asset_rotation(config.iterations, config.tls_rotation)?,
        run_high_frequency_apply(config.iterations, config.xds_apply).await?,
        run_last_good_fallback(config.iterations, config.xds_apply).await?,
    ];
    let comparisons = build_comparisons(&scenarios);

    Ok(BenchReport {
        captured_at_unix_seconds: unix_seconds_now(),
        allocator: ntgw_allocator::selected_allocator().to_string(),
        config,
        scenarios,
        comparisons,
    })
}

fn build_comparisons(scenarios: &[ScenarioReport]) -> Vec<ScenarioComparisonReport> {
    let mut comparisons = Vec::new();

    if let (Some(baseline), Some(current)) = (
        scenario_by_name(scenarios, "request_meta_header_heavy"),
        scenario_by_name(scenarios, "request_view_header_heavy"),
    ) {
        comparisons.push(scenario_comparison(
            "request_view_vs_meta_header_heavy",
            baseline,
            current,
            json!({
                "fixture": "header-heavy request",
                "baseline_path": "owned RequestMeta materialization",
                "current_path": "RequestView context capture before lazy materialization",
                "same_request_shape": true,
            }),
        ));
    }

    if let (Some(baseline), Some(current)) = (
        scenario_by_name(scenarios, "request_meta_header_heavy"),
        scenario_by_name(scenarios, "request_fast_path_selection"),
    ) {
        comparisons.push(scenario_comparison(
            "request_fast_path_vs_meta_header_heavy",
            baseline,
            current,
            json!({
                "fixture": "simple HTTPRoute request",
                "baseline_path": "owned RequestMeta materialization plus generic selection",
                "current_path": "borrowed request view plus compiled fast path selection",
                "production_path_changed": true,
            }),
        ));
    }

    if let Some(comparison) = snapshot_read_comparison(scenarios) {
        comparisons.push(comparison);
    }

    if let Some(comparison) = access_log_disabled_comparison(scenarios) {
        comparisons.push(comparison);
    }

    if let Some(comparison) = access_log_sampled_out_comparison(scenarios) {
        comparisons.push(comparison);
    }

    if let Some(comparison) = pool_contention_comparison(scenarios) {
        comparisons.push(comparison);
    }

    comparisons
}

fn pool_contention_comparison(scenarios: &[ScenarioReport]) -> Option<ScenarioComparisonReport> {
    let baseline = scenario_by_name(scenarios, "stream_pool_contention_spread")?;
    let current = scenario_by_name(scenarios, "stream_pool_contention_hot_key")?;
    Some(scenario_comparison(
        "pool_contention_hot_key_vs_spread",
        baseline,
        current,
        json!({
            "fixture": "concurrent tcp pool get/return, prewarmed reuse path",
            "baseline_path": "distinct backend per worker (separate DashMap shards)",
            "current_path": "single hot backend shared by all workers (one shard lock held across try_read)",
            "production_path_changed": false,
        }),
    ))
}

fn access_log_disabled_comparison(
    scenarios: &[ScenarioReport],
) -> Option<ScenarioComparisonReport> {
    let baseline = scenario_by_name(scenarios, "access_log_write_path")?;
    let current = scenario_by_name(scenarios, "access_log_disabled_path")?;
    Some(scenario_comparison(
        "access_log_disabled_vs_full_write",
        baseline,
        current,
        json!({
            "fixture": "access log hot path",
            "baseline_path": "json render plus background writer enqueue and flush",
            "current_path": "disabled access log fast path",
            "production_path_changed": false,
        }),
    ))
}

fn access_log_sampled_out_comparison(
    scenarios: &[ScenarioReport],
) -> Option<ScenarioComparisonReport> {
    let baseline = scenario_by_name(scenarios, "access_log_write_path")?;
    let current = scenario_by_name(scenarios, "access_log_sampled_out_path")?;
    Some(scenario_comparison(
        "access_log_sampled_out_vs_full_write",
        baseline,
        current,
        json!({
            "fixture": "access log hot path",
            "baseline_path": "json render plus background writer enqueue and flush",
            "current_path": "sampled-out access log fast path",
            "production_path_changed": false,
        }),
    ))
}

fn snapshot_read_comparison(scenarios: &[ScenarioReport]) -> Option<ScenarioComparisonReport> {
    let baseline = scenario_by_name(scenarios, "snapshot_read_rwlock")?;
    let current = scenario_by_name(scenarios, "snapshot_read_arc_swap")?;
    Some(scenario_comparison(
        "arc_swap_vs_rwlock_snapshot_read",
        baseline,
        current,
        json!({
            "fixture": "shared snapshot read-only hot path",
            "baseline_path": "Arc<RwLock<Snapshot>>::read",
            "current_path": "ArcSwap<Snapshot>::load",
            "production_path_changed": false,
        }),
    ))
}

fn scenario_comparison(
    name: &str,
    baseline: &ScenarioReport,
    current: &ScenarioReport,
    details: serde_json::Value,
) -> ScenarioComparisonReport {
    ScenarioComparisonReport {
        name: name.to_string(),
        baseline: baseline.name.clone(),
        current: current.name.clone(),
        timing_delta: TimingDelta::between(&baseline.timing, &current.timing),
        resource_delta: ResourceDelta::compare(&baseline.resource_delta, &current.resource_delta),
        details,
    }
}

fn scenario_by_name<'a>(scenarios: &'a [ScenarioReport], name: &str) -> Option<&'a ScenarioReport> {
    scenarios.iter().find(|scenario| scenario.name == name)
}

impl TimingDelta {
    fn between(baseline: &TimingSummary, current: &TimingSummary) -> Self {
        Self {
            average_ms: current.average_ms - baseline.average_ms,
            p50_ms: current.p50_ms - baseline.p50_ms,
            p95_ms: current.p95_ms - baseline.p95_ms,
            p99_ms: current.p99_ms - baseline.p99_ms,
            max_ms: current.max_ms - baseline.max_ms,
            p99_reduction_ratio: reduction_ratio(baseline.p99_ms, current.p99_ms),
        }
    }
}

fn reduction_ratio(baseline: f64, current: f64) -> Option<f64> {
    if baseline <= 0.0 {
        return None;
    }
    Some((baseline - current) / baseline)
}

impl ResourceDelta {
    pub(crate) fn between(before: &ResourceSample, after: &ResourceSample) -> Self {
        Self {
            fd_count: delta(before.fd_count, after.fd_count),
            rss_kib: delta(before.rss_kib, after.rss_kib),
            threads: delta(before.threads, after.threads),
            cpu_user_ticks: delta(before.cpu_user_ticks, after.cpu_user_ticks),
            cpu_system_ticks: delta(before.cpu_system_ticks, after.cpu_system_ticks),
            allocations: delta(before.allocations, after.allocations),
            deallocations: delta(before.deallocations, after.deallocations),
            reallocations: delta(before.reallocations, after.reallocations),
            bytes_allocated: delta(before.bytes_allocated, after.bytes_allocated),
            bytes_deallocated: delta(before.bytes_deallocated, after.bytes_deallocated),
            bytes_reallocated: signed_delta(before.bytes_reallocated, after.bytes_reallocated),
        }
    }

    fn compare(baseline: &Self, current: &Self) -> Self {
        Self {
            fd_count: signed_option_delta(baseline.fd_count, current.fd_count),
            rss_kib: signed_option_delta(baseline.rss_kib, current.rss_kib),
            threads: signed_option_delta(baseline.threads, current.threads),
            cpu_user_ticks: signed_option_delta(baseline.cpu_user_ticks, current.cpu_user_ticks),
            cpu_system_ticks: signed_option_delta(
                baseline.cpu_system_ticks,
                current.cpu_system_ticks,
            ),
            allocations: signed_option_delta(baseline.allocations, current.allocations),
            deallocations: signed_option_delta(baseline.deallocations, current.deallocations),
            reallocations: signed_option_delta(baseline.reallocations, current.reallocations),
            bytes_allocated: signed_option_delta(baseline.bytes_allocated, current.bytes_allocated),
            bytes_deallocated: signed_option_delta(
                baseline.bytes_deallocated,
                current.bytes_deallocated,
            ),
            bytes_reallocated: signed_option_delta(
                baseline.bytes_reallocated,
                current.bytes_reallocated,
            ),
        }
    }
}

pub(crate) fn summarize_durations(samples_ms: &[f64]) -> TimingSummary {
    let mut sorted = samples_ms.to_vec();
    sorted.sort_by(f64::total_cmp);
    let total_ms = samples_ms.iter().sum::<f64>();
    let average_ms = if samples_ms.is_empty() {
        0.0
    } else {
        total_ms / samples_ms.len() as f64
    };

    TimingSummary {
        total_ms,
        average_ms,
        p50_ms: percentile(&sorted, 0.50),
        p95_ms: percentile(&sorted, 0.95),
        p99_ms: percentile(&sorted, 0.99),
        max_ms: sorted.last().copied().unwrap_or_default(),
    }
}

fn percentile(sorted_samples: &[f64], quantile: f64) -> f64 {
    if sorted_samples.is_empty() {
        return 0.0;
    }

    let index = ((sorted_samples.len() - 1) as f64 * quantile).round() as usize;
    sorted_samples[index.min(sorted_samples.len() - 1)]
}

pub(crate) fn sample_resources() -> ResourceSample {
    let mut sample = ResourceSample {
        fd_count: fs::read_dir("/proc/self/fd")
            .ok()
            .map(|entries| entries.count() as u64),
        ..ResourceSample::default()
    };

    if let Ok(status) = fs::read_to_string("/proc/self/status") {
        for line in status.lines() {
            if let Some(value) = parse_status_value(line, "VmRSS:") {
                sample.rss_kib = Some(value);
            }
            if let Some(value) = parse_status_value(line, "Threads:") {
                sample.threads = Some(value);
            }
        }
    }
    if let Ok(stat) = fs::read_to_string("/proc/self/stat")
        && let Some((user_ticks, system_ticks)) = parse_proc_stat_cpu_ticks(&stat)
    {
        sample.cpu_user_ticks = Some(user_ticks);
        sample.cpu_system_ticks = Some(system_ticks);
    }

    apply_allocation_stats(&mut sample);

    sample
}

#[cfg(all(
    not(feature = "allocator-mimalloc"),
    not(feature = "allocator-jemalloc")
))]
fn apply_allocation_stats(sample: &mut ResourceSample) {
    let stats = stats_alloc::INSTRUMENTED_SYSTEM.stats();
    sample.allocations = Some(stats.allocations as u64);
    sample.deallocations = Some(stats.deallocations as u64);
    sample.reallocations = Some(stats.reallocations as u64);
    sample.bytes_allocated = Some(stats.bytes_allocated as u64);
    sample.bytes_deallocated = Some(stats.bytes_deallocated as u64);
    sample.bytes_reallocated = Some(stats.bytes_reallocated as i64);
}

#[cfg(any(feature = "allocator-mimalloc", feature = "allocator-jemalloc"))]
fn apply_allocation_stats(_sample: &mut ResourceSample) {}

fn parse_proc_stat_cpu_ticks(stat: &str) -> Option<(u64, u64)> {
    let rest = stat.rsplit_once(") ")?.1;
    let fields = rest.split_whitespace().collect::<Vec<_>>();
    let user_ticks = fields.get(11)?.parse::<u64>().ok()?;
    let system_ticks = fields.get(12)?.parse::<u64>().ok()?;
    Some((user_ticks, system_ticks))
}

fn parse_status_value(line: &str, key: &str) -> Option<u64> {
    line.strip_prefix(key)
        .and_then(|value| value.split_whitespace().next())
        .and_then(|value| value.parse::<u64>().ok())
}

fn delta(before: Option<u64>, after: Option<u64>) -> Option<i64> {
    Some(after? as i64 - before? as i64)
}

fn signed_delta(before: Option<i64>, after: Option<i64>) -> Option<i64> {
    Some(after? - before?)
}

fn signed_option_delta(baseline: Option<i64>, current: Option<i64>) -> Option<i64> {
    Some(current? - baseline?)
}

pub(crate) fn elapsed_ms(duration: std::time::Duration) -> f64 {
    duration.as_secs_f64() * 1000.0
}

pub(crate) fn unique_temp_dir(prefix: &str) -> PathBuf {
    std::env::temp_dir().join(format!("{prefix}-{}", unix_nanos_now()))
}

fn unix_seconds_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn unix_nanos_now() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos()
}
