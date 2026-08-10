use super::*;
use std::sync::Arc;

use pingora::{
    apps::ServerApp, protocols::Stream, proxy::HttpProxy, server::configuration::ServerConf,
};

use crate::proxy::{GatewayProxyOptions, UpstreamTuningOptions};

mod filters;
mod langfuse;
mod service;
mod tls;

use super::capacity::{effective_http_capacity, server_conf_for_capacity};

#[cfg(test)]
pub(super) use service::plain_http_server_options;

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
    let opts = GatewayProxyOptions {
        snapshot,
        access_log,
        session_persistence,
        traffic,
        admission,
        circuit_breaker,
        rate_limit,
        retry_budget,
        downstream_read_timeout: runtime.downstream_read_timeout,
        downstream_max_connection_age: runtime.downstream_max_connection_age,
        upstream_tcp_keepalive: runtime.upstream_tcp_keepalive.clone(),
        upstream_tuning: upstream_tuning_from_runtime(&runtime),
        request_tracing_enabled: runtime.request_tracing_enabled,
        max_request_body_bytes: runtime.max_request_body_bytes,
        max_request_header_bytes: runtime.max_request_header_bytes,
        ai_gateway_max_request_body_bytes: runtime.experimental.ai_gateway_max_request_body_bytes,
        listener_name_hint: None,
        listener_port_hint: None,
        cache: runtime.cache.clone(),
        wasm_filter: None,
        ai_filter: None,
    };
    let join = thread::spawn(move || run_server(plan_for_thread, runtime, opts, receiver));

    Ok(ActiveServer {
        plan,
        shutdown,
        join,
        asset_dir: Some(asset_dir),
    })
}

pub(super) fn start_server_with_asset_root(
    plan: ListenerPlan,
    opts: GatewayProxyOptions,
    runtime: RuntimeOptions,
    asset_root: &Path,
    stage_recorder: Option<&dyn ntgw_observability::ApplyStageRecorder>,
) -> Result<(ActiveServer, TlsAssetWriteStats)> {
    let (shutdown, receiver) = watch::channel(false);
    let stage = std::time::Instant::now();
    let asset_stats = materialize_tls_assets_in_dir(&plan, asset_root)?;
    observe_reload_stage_elapsed(stage_recorder, "tls_assets", stage);
    let plan_for_thread = materialize_runtime_plan(&plan, asset_root);
    let join = thread::spawn(move || run_server(plan_for_thread, runtime, opts, receiver));

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

#[tracing::instrument(skip_all)]
pub fn build_http_app(
    opts: GatewayProxyOptions,
    runtime: RuntimeOptions,
) -> Result<AcceptedHttpApp> {
    let mut app = HttpProxy::new(
        build_gateway_proxy(&opts, &runtime),
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
    if ServerApp::process_new(&app.inner, stream, &shutdown)
        .await
        .is_none()
    {
        tracing::debug!("HTTP connection spawn failed");
    }
    Ok(())
}

#[tracing::instrument(skip_all)]
fn run_server(
    plan: RuntimePlan,
    runtime: RuntimeOptions,
    opts: GatewayProxyOptions,
    shutdown: watch::Receiver<bool>,
) {
    let mut server = Server::new_with_opt_and_conf(None, server_conf_for_runtime(&runtime));

    server.bootstrap();

    if let Err(err) = service::add_plain_http_service(&mut server, &plan, &opts, &runtime) {
        error!(error = %err, "failed to configure plain http listeners");
        return;
    }
    if let Err(err) = tls::add_tls_http_service(&mut server, &plan, &opts, &runtime) {
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

fn build_gateway_proxy(opts: &GatewayProxyOptions, runtime: &RuntimeOptions) -> GatewayProxy {
    let mut proxy_opts = (*opts).clone();
    proxy_opts.wasm_filter = if runtime.experimental.enable_experimental_gateway {
        filters::build_wasm_filter(&proxy_opts.snapshot)
    } else {
        None
    };
    proxy_opts.ai_filter = if runtime.experimental.enable_ai_gateway {
        filters::build_ai_filter(&proxy_opts.snapshot, proxy_opts.wasm_filter.clone())
    } else {
        None
    };
    GatewayProxy::new(proxy_opts)
}

#[allow(dead_code)]
pub(super) fn listener_port_hint(listeners: &[&RuntimeListener]) -> Option<u32> {
    let mut ports = listeners
        .iter()
        .map(|listener| listener_bind_port(listener.bind.as_str()));
    let first = ports.next()??;
    ports.all(|port| port == Some(first)).then_some(first)
}
#[allow(dead_code)]
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

pub(super) fn upstream_tuning_from_runtime(runtime: &RuntimeOptions) -> UpstreamTuningOptions {
    UpstreamTuningOptions {
        tcp_fast_open: runtime.upstream_tcp_fast_open,
        tcp_recv_buf: runtime.upstream_tcp_recv_buf,
        connection_timeout: runtime.upstream_connection_timeout,
        read_timeout: runtime.upstream_read_timeout,
        idle_timeout: runtime.upstream_idle_timeout,
        dscp: runtime.upstream_dscp,
    }
}
