#![forbid(unsafe_code)]

mod active_health;
mod admin;
mod config_mapping;
mod config_reload;
mod supervisor;
mod xds_runtime;

use parking_lot::RwLock;
use std::{net::SocketAddr, sync::Arc};

use anyhow::{Result, anyhow};
use clap::Parser;
use tokio::net::TcpListener;
use tokio::sync::{mpsc, watch};
use tokio::time::sleep;
use tracing::{error, info};

use admin::{AppState, build_router};
use config_mapping::{to_sentry_options, to_tracing_options};
use config_reload::{
    CONFIG_RELOAD_POLL_INTERVAL, ReloadTargets, build_config_snapshot, spawn_config_reload_loop,
};
use ntgw_config::ReloadingDataPlaneConfig;
use ntgw_ir::{Snapshot, SnapshotSignal};
use ntgw_observability::{
    AdminRequestStats, HttpCircuitBreakerController, HttpRateLimitController, OverloadStats,
    RetryBudgetController, RuntimeStats, SharedApplyStageRecorder, SharedRuntimeStats,
    SharedTrafficStats, UdpSessionStats, init_sentry, init_tracing,
};
use ntgw_xds::ClientStats;
use supervisor::{
    ShutdownCause, ShutdownCoordinator, wait_for_shutdown, wait_for_termination_signal,
};
use xds_runtime::run_xds_loop;

#[derive(Parser, Debug)]
struct Cli {
    #[arg(long, default_value = "../configs/dataplane/config.yaml")]
    config: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    let config_source = ReloadingDataPlaneConfig::new(&cli.config, CONFIG_RELOAD_POLL_INTERVAL)?;
    let cfg = config_source.load()?;
    let sentry_options = to_sentry_options(&cfg);
    init_tracing(&to_tracing_options(&cfg), Some(&sentry_options))?;
    let _sentry_guard = init_sentry(&sentry_options, env!("CARGO_PKG_VERSION"))?;
    let admin_auth_configured = cfg.admin_auth.resolve_bearer_token()?.is_some();
    validate_admin_auth_exposure(&cfg.admin_addr, admin_auth_configured)?;
    let initial_config = build_config_snapshot(&cfg)?;
    ntgw_http::configure_request_mirror_budget(initial_config.request_mirror_max_concurrency);

    let snapshot = Snapshot::shared();
    let snapshot_updates = SnapshotSignal::shared();
    let xds = ClientStats::shared();
    let apply_stage_recorder: SharedApplyStageRecorder = xds.clone();
    let runtime_stats = RuntimeStats::shared();
    let xds_runtime = runtime_stats.clone();
    let traffic = SharedTrafficStats::shared();
    let udp_sessions = UdpSessionStats::shared();
    let admin_requests = AdminRequestStats::shared();
    let overload = OverloadStats::shared();
    let admin_config = Arc::new(RwLock::new(initial_config.admin.clone()));
    let circuit_breaker = Arc::new(RwLock::new(HttpCircuitBreakerController::new(
        initial_config.circuit_breaker.clone(),
    )));
    let rate_limit = Arc::new(RwLock::new(HttpRateLimitController::new(
        initial_config.rate_limit.clone(),
    )));
    let retry_budget = Arc::new(RwLock::new(RetryBudgetController::new(
        initial_config.retry_budget.clone(),
    )));
    let (http_config_tx, http_config_rx) = watch::channel(Arc::new(initial_config.http.clone()));
    let (shared_tls_config_tx, shared_tls_config_rx) =
        watch::channel(Arc::new(initial_config.shared_tls.clone()));
    let (stream_config_tx, stream_config_rx) =
        watch::channel(Arc::new(initial_config.stream.clone()));
    let (xds_config_tx, xds_config_rx) = watch::channel(Arc::new(initial_config.xds.clone()));
    let (active_health_config_tx, active_health_config_rx) =
        watch::channel(Arc::new(initial_config.active_health.clone()));
    let graceful_drain_period = cfg.runtime_tuning.graceful_drain_period();
    let state = AppState {
        config: admin_config.clone(),
        snapshot: snapshot.clone(),
        runtime: runtime_stats.clone(),
        traffic: traffic.clone(),
        udp_sessions: udp_sessions.clone(),
        admin_requests: admin_requests.clone(),
        xds: xds.clone(),
        overload: overload.clone(),
        circuit_breaker: circuit_breaker.clone(),
        rate_limit: rate_limit.clone(),
        retry_budget: retry_budget.clone(),
    };
    let supervisor = ShutdownCoordinator::new();
    runtime_stats.observe_supervisor_started();
    let (shutdown_events_tx, mut shutdown_events_rx) = mpsc::unbounded_channel::<ShutdownCause>();
    let http_shutdown = supervisor.subscribe();
    let xds_shutdown = supervisor.subscribe();
    let active_health_shutdown = supervisor.subscribe();
    let shared_tls_shutdown = supervisor.subscribe();
    let stream_shutdown = supervisor.subscribe();
    let admin_shutdown = supervisor.subscribe();
    let config_reload_shutdown = supervisor.subscribe();

    let http_handle = ntgw_http::spawn(
        snapshot.clone(),
        snapshot_updates.clone(),
        http_config_rx,
        runtime_stats.clone(),
        traffic.clone(),
        overload.clone(),
        circuit_breaker.clone(),
        rate_limit.clone(),
        retry_budget.clone(),
        Some(apply_stage_recorder.clone()),
        http_shutdown,
    )?;
    runtime_stats.observe_http_runtime_started();
    let http_runtime = runtime_stats.clone();
    let http_shutdown_events = shutdown_events_tx.clone();
    let http_join = tokio::task::spawn_blocking(move || match http_handle.join() {
        Ok(()) => {
            http_runtime.observe_http_runtime_exited("http runtime exited");
            let _ = http_shutdown_events.send(ShutdownCause::fatal("http runtime exited"));
        }
        Err(_) => {
            http_runtime.observe_http_runtime_exited("http runtime panicked");
            let _ = http_shutdown_events.send(ShutdownCause::fatal("http runtime panicked"));
        }
    });

    let xds_snapshot = snapshot.clone();
    let xds_updates = snapshot_updates.clone();
    let xds_stats = xds.clone();
    let xds_shutdown_events = shutdown_events_tx.clone();
    let xds_shutdown_observer = xds_shutdown.clone();
    let xds_task = tokio::spawn(async move {
        run_xds_loop(
            xds_config_rx,
            xds_snapshot,
            xds_updates,
            xds_runtime,
            xds_stats,
            xds_shutdown,
        )
        .await;
        if !*xds_shutdown_observer.borrow() {
            let _ = xds_shutdown_events
                .send(ShutdownCause::fatal("xds supervisor exited unexpectedly"));
        }
    });

    let active_health_shutdown_observer = active_health_shutdown.clone();
    let active_health_handle = active_health::spawn(
        snapshot.clone(),
        active_health_config_rx,
        active_health_shutdown,
    );
    let active_health_shutdown_events = shutdown_events_tx.clone();
    let active_health_task = tokio::spawn(async move {
        match active_health_handle.await {
            Ok(()) => {
                if !*active_health_shutdown_observer.borrow() {
                    let _ = active_health_shutdown_events.send(ShutdownCause::fatal(
                        "active health task exited unexpectedly",
                    ));
                }
            }
            Err(err) => {
                let _ = active_health_shutdown_events.send(ShutdownCause::fatal(format!(
                    "active health task panicked: {err}"
                )));
            }
        }
    });

    let stream_snapshot = snapshot.clone();
    let stream_updates = snapshot_updates.clone();
    let stream_runtime = runtime_stats.clone();
    let stream_traffic = traffic.clone();
    let stream_udp_sessions = udp_sessions.clone();
    let stream_overload = overload.clone();
    let stream_shutdown_events = shutdown_events_tx.clone();
    let stream_shutdown_observer = stream_shutdown.clone();
    let stream_task = tokio::spawn(async move {
        stream_runtime.observe_stream_runtime_started();
        let stream_runtime_for_run = stream_runtime.clone();
        if let Err(err) = ntgw_stream::run(
            stream_snapshot,
            stream_updates,
            stream_config_rx,
            stream_runtime_for_run,
            stream_traffic,
            stream_udp_sessions,
            stream_overload,
            stream_shutdown,
        )
        .await
        {
            let error_message = err.to_string();
            stream_runtime.observe_stream_runtime_exited(&error_message);
            error!(error = %err, "stream runtime exited");
            if !*stream_shutdown_observer.borrow() {
                let _ = stream_shutdown_events.send(ShutdownCause::fatal(error_message));
            }
            return;
        }

        stream_runtime.observe_stream_runtime_exited("stream runtime exited");
        if !*stream_shutdown_observer.borrow() {
            let _ = stream_shutdown_events
                .send(ShutdownCause::fatal("stream runtime exited unexpectedly"));
        }
    });

    let shared_tls_snapshot = snapshot.clone();
    let shared_tls_updates = snapshot_updates.clone();
    let shared_tls_runtime = runtime_stats.clone();
    let shared_tls_traffic = traffic.clone();
    let shared_tls_overload = overload.clone();
    let shared_tls_shutdown_events = shutdown_events_tx.clone();
    let shared_tls_shutdown_observer = shared_tls_shutdown.clone();
    let shared_tls_circuit_breaker = circuit_breaker.clone();
    let shared_tls_rate_limit = rate_limit.clone();
    let shared_tls_retry_budget = retry_budget.clone();
    let shared_tls_task = tokio::spawn(async move {
        shared_tls_runtime.observe_tls_runtime_started();
        let shared_tls_runtime_for_run = shared_tls_runtime.clone();
        if let Err(err) = ntgw_shared_tls::run(
            shared_tls_snapshot,
            shared_tls_updates,
            shared_tls_config_rx,
            shared_tls_runtime_for_run,
            shared_tls_traffic,
            shared_tls_overload,
            shared_tls_circuit_breaker,
            shared_tls_rate_limit,
            shared_tls_retry_budget,
            Some(apply_stage_recorder.clone()),
            shared_tls_shutdown,
        )
        .await
        {
            let error_message = err.to_string();
            shared_tls_runtime.observe_tls_runtime_exited(&error_message);
            error!(error = %err, "shared tls runtime exited");
            if !*shared_tls_shutdown_observer.borrow() {
                let _ = shared_tls_shutdown_events.send(ShutdownCause::fatal(error_message));
            }
            return;
        }

        shared_tls_runtime.observe_tls_runtime_exited("shared tls runtime exited");
        if !*shared_tls_shutdown_observer.borrow() {
            let _ = shared_tls_shutdown_events.send(ShutdownCause::fatal(
                "shared tls runtime exited unexpectedly",
            ));
        }
    });

    let config_reload_shutdown_observer = config_reload_shutdown.clone();
    let config_reload_handle = spawn_config_reload_loop(
        config_source,
        ReloadTargets {
            admin: admin_config,
            http: http_config_tx,
            shared_tls: shared_tls_config_tx,
            stream: stream_config_tx,
            xds: xds_config_tx,
            active_health: active_health_config_tx,
            circuit_breaker: circuit_breaker.clone(),
            rate_limit: rate_limit.clone(),
            retry_budget: retry_budget.clone(),
        },
        config_reload_shutdown,
    );
    let config_reload_shutdown_events = shutdown_events_tx.clone();
    let config_reload_task = tokio::spawn(async move {
        match config_reload_handle.await {
            Ok(()) => {
                if !*config_reload_shutdown_observer.borrow() {
                    let _ = config_reload_shutdown_events.send(ShutdownCause::fatal(
                        "config reload task exited unexpectedly",
                    ));
                }
            }
            Err(err) => {
                let _ = config_reload_shutdown_events.send(ShutdownCause::fatal(format!(
                    "config reload task panicked: {err}"
                )));
            }
        }
    });

    let router = build_router(state);

    let addr: SocketAddr = cfg.admin_addr.parse()?;
    let listener = TcpListener::bind(addr).await?;

    info!(
        admin = %cfg.admin_addr,
        allocator = ntgw_allocator::selected_allocator(),
        control_plane = %cfg.control_plane_addr,
        xds_tls = cfg.xds_tls.enabled(),
        "dataplane started"
    );
    if !admin_auth_configured {
        tracing::warn!("admin API is running without bearer token authentication");
    }
    let admin_shutdown_events = shutdown_events_tx.clone();
    let admin_task = tokio::spawn(async move {
        let result = axum::serve(listener, router)
            .with_graceful_shutdown(wait_for_shutdown(admin_shutdown))
            .await;
        match &result {
            Ok(()) => {
                let _ = admin_shutdown_events.send(ShutdownCause::fatal("admin server exited"));
            }
            Err(err) => {
                let _ = admin_shutdown_events
                    .send(ShutdownCause::fatal(format!("admin server exited: {err}")));
            }
        }
        result
    });
    let signal_shutdown_events = shutdown_events_tx.clone();
    let signal_task = tokio::spawn(async move {
        let _ = signal_shutdown_events.send(wait_for_termination_signal().await);
    });

    let shutdown_cause = shutdown_events_rx
        .recv()
        .await
        .unwrap_or_else(|| ShutdownCause::fatal("supervisor event channel closed"));
    initiate_runtime_shutdown(
        runtime_stats.clone(),
        supervisor.clone(),
        shutdown_cause.clone(),
        graceful_drain_period,
    )
    .await;

    signal_task.abort();
    if let Err(e) = admin_task.await {
        error!(%e, "admin task panicked during shutdown");
    }
    if let Err(e) = shared_tls_task.await {
        error!(%e, "shared-tls task panicked during shutdown");
    }
    if let Err(e) = stream_task.await {
        error!(%e, "stream task panicked during shutdown");
    }
    if let Err(e) = xds_task.await {
        error!(%e, "xds task panicked during shutdown");
    }
    if let Err(e) = active_health_task.await {
        error!(%e, "active health task panicked during shutdown");
    }
    if let Err(e) = config_reload_task.await {
        error!(%e, "config reload task panicked during shutdown");
    }
    if let Err(e) = http_join.await {
        error!(%e, "http task panicked during shutdown");
    }

    let shutdown_message = if shutdown_cause.graceful {
        "graceful shutdown complete".to_string()
    } else {
        format!("terminated after {}", shutdown_cause.reason)
    };
    runtime_stats.observe_supervisor_exited(shutdown_message.as_str());

    if shutdown_cause.graceful {
        return Ok(());
    }

    Err(anyhow!(shutdown_cause.reason))
}

fn validate_admin_auth_exposure(admin_addr: &str, bearer_auth_configured: bool) -> Result<()> {
    let addr: SocketAddr = admin_addr.parse()?;
    if bearer_auth_configured || addr.ip().is_loopback() {
        return Ok(());
    }

    Err(anyhow!(
        "admin API bound to {admin_addr} requires bearer token authentication"
    ))
}

async fn initiate_runtime_shutdown(
    runtime_stats: SharedRuntimeStats,
    supervisor: ShutdownCoordinator,
    shutdown_cause: ShutdownCause,
    graceful_drain_period: std::time::Duration,
) {
    runtime_stats.observe_supervisor_shutdown_requested(shutdown_cause.reason.as_str());
    if shutdown_cause.graceful && !graceful_drain_period.is_zero() {
        sleep(graceful_drain_period).await;
    }
    supervisor.request_shutdown();
}

#[cfg(test)]
mod tests;
