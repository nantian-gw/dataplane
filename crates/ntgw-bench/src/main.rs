#![forbid(unsafe_code)]

use std::{fs, path::PathBuf};

use anyhow::Result;
use clap::Parser;

mod report;
mod scenarios;
#[cfg(test)]
mod tests;

use report::{BenchConfig, build_report};

#[cfg(all(
    not(feature = "allocator-mimalloc"),
    not(feature = "allocator-jemalloc")
))]
#[global_allocator]
static GLOBAL_ALLOCATOR: &stats_alloc::StatsAlloc<std::alloc::System> =
    &stats_alloc::INSTRUMENTED_SYSTEM;

#[derive(Parser, Debug, Clone)]
struct Cli {
    #[arg(long)]
    output: Option<PathBuf>,
    #[arg(long, default_value_t = 25)]
    iterations: u32,
    #[arg(long, default_value_t = 24)]
    snapshot_listeners: usize,
    #[arg(long, default_value_t = 16)]
    routes_per_listener: usize,
    #[arg(long, default_value_t = 4)]
    backends_per_route: usize,
    #[arg(long, default_value_t = 4)]
    endpoints_per_backend: usize,
    #[arg(long, default_value_t = 32)]
    tls_listeners: usize,
    #[arg(long, default_value_t = 4)]
    tls_ca_bundle_variants: usize,
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    let iterations = cli.iterations.max(1);
    let snapshot_config = ntgw_ir::bench::SnapshotBenchConfig {
        listeners: cli.snapshot_listeners,
        routes_per_listener: cli.routes_per_listener,
        backends_per_route: cli.backends_per_route,
        endpoints_per_backend: cli.endpoints_per_backend,
    };
    let tls_config = ntgw_http::runtime_bench::TlsRotationBenchConfig {
        listeners: cli.tls_listeners,
        ca_bundle_variants: cli.tls_ca_bundle_variants,
    };
    let xds_config = ntgw_xds::bench::ApplyBenchConfig::default();
    let request_meta_config = ntgw_http::bench::RequestMetaBuildBenchConfig::default();
    let filter_chain_config = ntgw_http::bench::FilterChainBenchConfig::default();
    let session_config = ntgw_http::bench::SessionBenchConfig::default();
    let access_log_config = ntgw_observability::bench::AccessLogBenchConfig::default();
    let traffic_stats_config = ntgw_observability::bench::TrafficStatsBenchConfig::default();
    let http_capacity_config = ntgw_http::runtime_bench::HttpCapacityMatrixBenchConfig::default();
    let stream_config = ntgw_stream::bench::StreamBenchConfig::default();
    let report = build_report(BenchConfig {
        iterations,
        snapshot: snapshot_config,
        tls_rotation: tls_config,
        xds_apply: xds_config,
        request_meta: request_meta_config,
        filter_chain: filter_chain_config,
        session_persistence: session_config,
        access_log: access_log_config,
        traffic_stats: traffic_stats_config,
        http_capacity: http_capacity_config,
        stream: stream_config,
    })
    .await?;

    let rendered = serde_json::to_string_pretty(&report)?;
    if let Some(path) = cli.output.as_ref() {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(path, rendered)?;
    } else {
        println!("{rendered}");
    }

    Ok(())
}
