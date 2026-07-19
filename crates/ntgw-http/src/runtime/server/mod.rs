use super::*;
use std::sync::Arc;

use pingora::{
    apps::ServerApp, protocols::Stream, proxy::HttpProxy, server::configuration::ServerConf,
};

use crate::cache::CacheManager;
use crate::proxy::UpstreamTuningOptions;

mod tls;
mod filters;
mod langfuse;
mod service;

use super::capacity::{effective_http_capacity, server_conf_for_capacity};

#[cfg(test)]
pub(super) fn start_server(
    plan: ListenerPlan,
    snapshot: SharedSnapshot,
    runtime: RuntimeOptions,
    access_log: AccessLogOptions,
    session_persistence: SessionPersistenceOptions,
    traffic: SharedTrafficStats,
) -> Result<ActiveServer> {
    start_server_with_overload_stats(
        plan,
        snapshot,
        runtime,
        access_log,
        session_persistence,
        traffic,
        ntgw_observability::OverloadStats::shared(),
    )
}

#[cfg(test)]
pub(super) fn start_server_with_overload_stats(
    plan: ListenerPlan,
    snapshot: SharedSnapshot,
    runtime: RuntimeOptions,
    access_log: AccessLogOptions,
    session_persistence: SessionPersistenceOptions,
    traffic: SharedTrafficStats,
    overload: SharedOverloadStats,
) -> Result<ActiveServer> {
    let asset_dir = materialize_tls_assets(&plan)?;
    let plan_for_thread = materialize_runtime_plan(&plan, &asset_dir);
    let (shutdown, receiver) = watch::channel(false);
    let admission = HttpAdmissionController::new(runtime.admission.clone(), overload);
    let circuit_breaker = HttpCircuitBreakerController::new(runtime.circuit_breaker.clone());
    let rate_limit = HttpRateLimitController::new(runtime.rate_limit.clone());
    let retry_budget = RetryBudgetController::new(runtime.retry_budget.clone());
    let join = thread::spawn(move || {
        run_server(
            plan_for_thread,
            snapshot,
            runtime,
            access_log,
            session_persistence,
            traffic,
            admission,
            circuit_breaker,
            rate_limit,
            retry_budget,
            receiver,
        )
    });

    Ok(ActiveServer {
        plan,
        shutdown,
        join,
        asset_dir: Some(asset_dir),
    })
}

#[allow(clippy::too_many_arguments)]
pub(super) fn start_server_with_asset_root(
    plan: ListenerPlan,
    snapshot: SharedSnapshot,
    runtime: RuntimeOptions,
    access_log: AccessLogOptions,
    session_persistence: SessionPersistenceOptions,
    traffic: SharedTrafficStats,
    admission: HttpAdmissionController,
    circuit_breaker: HttpCircuitBreakerController,
    rate_limit: HttpRateLimitController,
    retry_budget: RetryBudgetController,
    asset_root: &Path,
    stage_recorder: Option<&dyn ntgw_observability::ApplyStageRecorder>,
) -> Result<(ActiveServer, TlsAssetWriteStats)> {
    let (shutdown, receiver) = watch::channel(false);
    let stage = std::time::Instant::now();
    let asset_stats = materialize_tls_assets_in_dir(&plan, asset_root)?;
    observe_reload_stage_elapsed(stage_recorder, "tls_assets", stage);
    let plan_for_thread = materialize_runtime_plan(&plan, asset_root);
    let join = thread::spawn(move || {
        run_server(
            plan_for_thread,
            snapshot,
            runtime,
            access_log,
            session_persistence,
            traffic,
            admission,
            circuit_breaker,
            rate_limit,
            retry_budget,
            receiver,
        )
    });

    Ok((
        ActiveServer {
            plan,
            shutdown,
            join,
            asset_dir: None,
        },
        asset_stats,
    ))
}

pub(super) fn stop_server(server: ActiveServer) {
    let _ = server.shutdown.send(true);
    let _ = server.join.join();
    if let Some(asset_dir) = server.asset_dir {
        let _ = fs::remove_dir_all(asset_dir);
    }
}

pub(super) fn server_conf_for_runtime(runtime: &RuntimeOptions) -> ServerConf {
    let capacity = effective_http_capacity(&runtime.capacity);
    let mut conf = server_conf_for_capacity(&capacity);
    conf.work_stealing = runtime.work_stealing;
    conf
}

fn reuse_port_for_runtime(runtime: &RuntimeOptions) -> Option<bool> {
    effective_http_capacity(&runtime.capacity).reuse_port
}

#[derive(Clone)]
pub struct AcceptedHttpApp {
    inner: Arc<HttpProxy<GatewayProxy>>,
}

#[allow(clippy::too_many_arguments)]
pub fn build_http_app(
    snapshot: SharedSnapshot,
    runtime: RuntimeOptions,
    access_log: AccessLogOptions,
    session_persistence: SessionPersistenceOptions,
    traffic: SharedTrafficStats,
    overload: SharedOverloadStats,
    circuit_breaker: HttpCircuitBreakerController,
    rate_limit: HttpRateLimitController,
    retry_budget: RetryBudgetController,
    listener_name_hint: Option<String>,
) -> Result<AcceptedHttpApp> {
    let admission = HttpAdmissionController::new(runtime.admission.clone(), overload);
    let mut app = HttpProxy::new(
        build_gateway_proxy(
            snapshot,
            access_log,
            session_persistence,
            traffic,
            admission,
            circuit_breaker,
            rate_limit,
            retry_budget,
            runtime.downstream_read_timeout,
            runtime.downstream_max_connection_age,
            runtime.upstream_tcp_keepalive.clone(),
            upstream_tuning_from_runtime(&runtime),
            runtime.request_tracing_enabled,
            runtime.max_request_body_bytes,
            runtime.max_request_header_bytes,
            listener_name_hint,
            None,
            runtime.cache.clone(),
            runtime.experimental.clone(),
        ),
        Arc::new(server_conf_for_runtime(&runtime)),
    );
    app.server_options = Some(HttpServerOptions::default());
    app.handle_init_modules();
    Ok(AcceptedHttpApp {
        inner: Arc::new(app),
    })
}

#[tracing::instrument(skip_all)]
pub async fn process_accepted_stream(
    app: AcceptedHttpApp,
    stream: Stream,
    shutdown: watch::Receiver<bool>,
) -> Result<()> {
    let _ = ServerApp::process_new(&app.inner, stream, &shutdown).await;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn run_server(
    plan: RuntimePlan,
    snapshot: SharedSnapshot,
    runtime: RuntimeOptions,
    access_log: AccessLogOptions,
    session_persistence: SessionPersistenceOptions,
    traffic: SharedTrafficStats,
    admission: HttpAdmissionController,
    circuit_breaker: HttpCircuitBreakerController,
    rate_limit: HttpRateLimitController,
    retry_budget: RetryBudgetController,
    shutdown: watch::Receiver<bool>,
) {
    let mut server = Server::new_with_opt_and_conf(None, server_conf_for_runtime(&runtime));

    server.bootstrap();

    if let Err(err) = service::add_plain_http_service(
        &mut server,
        &plan,
        snapshot.clone(),
        &runtime,
        access_log.clone(),
        session_persistence.clone(),
        traffic.clone(),
        admission.clone(),
        circuit_breaker.clone(),
        rate_limit.clone(),
        retry_budget.clone(),
    ) {
        error!(error = %err, "failed to configure plain http listeners");
        return;
    }
    if let Err(err) = tls::add_tls_http_service(
        &mut server,
        &plan,
        snapshot,
        &runtime,
        access_log,
        session_persistence,
        traffic,
        admission,
        circuit_breaker,
        rate_limit,
        retry_budget,
    ) {
        error!(error = %err, "failed to configure tls http listeners");
        return;
    }

    let bind_list: Vec<&str> = plan
        .listeners
        .iter()
        .map(|listener| listener.bind.as_str())
        .collect();
    info!(
        listeners = ?bind_list,
        http3_configured = runtime.enable_http3,
        http3_available = http3_available(),
        "starting nantian http runtime"
    );

    #[cfg(unix)]
    server.run(RunArgs {
        shutdown_signal: Box::new(LocalShutdownSignalWatch { receiver: shutdown }),
    });

    #[cfg(not(unix))]
    {
        let _ = shutdown;
        server.run(RunArgs::default());
    }
}

#[allow(clippy::too_many_arguments)]
fn build_gateway_proxy(
    snapshot: SharedSnapshot,
    access_log: AccessLogOptions,
    session_persistence: SessionPersistenceOptions,
    traffic: SharedTrafficStats,
    admission: HttpAdmissionController,
    circuit_breaker: HttpCircuitBreakerController,
    rate_limit: HttpRateLimitController,
    retry_budget: RetryBudgetController,
    downstream_read_timeout: Option<Duration>,
    downstream_max_connection_age: Option<Duration>,
    upstream_tcp_keepalive: Option<TcpKeepalive>,
    upstream_tuning: UpstreamTuningOptions,
    request_tracing_enabled: bool,
    max_request_body_bytes: usize,
    max_request_header_bytes: usize,
    listener_name_hint: Option<String>,
    listener_port_hint: Option<u32>,
    cache: Arc<CacheManager>,
    experimental: ntgw_config::ExperimentalConfig,
) -> GatewayProxy {
    let wasm_filter = if experimental.enable_experimental_gateway {
        filters::build_wasm_filter(&snapshot)
    } else {
        None
    };
    let ai_filter = if experimental.enable_ai_gateway {
        filters::build_ai_filter(&snapshot, wasm_filter.clone())
    } else {
        None
    };

    GatewayProxy::new(
        snapshot,
        access_log,
        session_persistence,
        traffic,
        admission,
        circuit_breaker,
        rate_limit,
        retry_budget,
        downstream_read_timeout,
        downstream_max_connection_age,
        upstream_tcp_keepalive,
        upstream_tuning,
        request_tracing_enabled,
        max_request_body_bytes,
        max_request_header_bytes,
        experimental.ai_gateway_max_request_body_bytes,
        listener_name_hint,
        listener_port_hint,
        cache,
        wasm_filter,
        ai_filter,
    )
}

fn listener_name_hint(listeners: &[&RuntimeListener]) -> Option<String> {
    let mut names = listeners.iter().map(|listener| listener.name.as_str());
    let first = names.next()?;
    names.all(|name| name == first).then(|| first.to_string())
}

pub(super) fn listener_port_hint(listeners: &[&RuntimeListener]) -> Option<u32> {
    let mut ports = listeners
        .iter()
        .map(|listener| listener_bind_port(listener.bind.as_str()));
    let first = ports.next()??;
    ports.all(|port| port == Some(first)).then_some(first)
}

fn listener_bind_port(bind: &str) -> Option<u32> {
    let (_, port) = bind.rsplit_once(':')?;
    port.parse::<u16>().ok().map(u32::from)
}

#[cfg(unix)]
struct LocalShutdownSignalWatch {
    receiver: watch::Receiver<bool>,
}

#[cfg(unix)]
#[async_trait]
impl ShutdownSignalWatch for LocalShutdownSignalWatch {
    async fn recv(&self) -> ShutdownSignal {
        let mut receiver = self.receiver.clone();
        loop {
            if *receiver.borrow() {
                return ShutdownSignal::FastShutdown;
            }
            if receiver.changed().await.is_err() {
                return ShutdownSignal::FastShutdown;
            }
        }
    }
}

fn upstream_tuning_from_runtime(runtime: &RuntimeOptions) -> UpstreamTuningOptions {
    UpstreamTuningOptions {
        tcp_fast_open: runtime.upstream_tcp_fast_open,
        tcp_recv_buf: runtime.upstream_tcp_recv_buf,
        connection_timeout: runtime.upstream_connection_timeout,
        read_timeout: runtime.upstream_read_timeout,
        idle_timeout: runtime.upstream_idle_timeout,
        dscp: runtime.upstream_dscp,
    }
}
