use super::*;
use std::sync::Arc;

use pingora::{
    apps::ServerApp, protocols::Stream, proxy::HttpProxy, server::configuration::ServerConf,
    tls::ssl::SslSessionCacheMode,
};

use crate::cache::CacheManager;
use crate::proxy::DownstreamTlsInfo;
use crate::proxy::UpstreamTuningOptions;

use super::capacity::{effective_http_capacity, server_conf_for_capacity};

struct DynamicTlsCertificates {
    listener_name: String,
    identities: Vec<TlsIdentity>,
}

#[async_trait]
impl pingora::listeners::TlsAccept for DynamicTlsCertificates {
    async fn certificate_callback(&self, ssl: &mut pingora::tls::ssl::SslRef) {
        let server_name = ssl
            .servername(pingora::tls::ssl::NameType::HOST_NAME)
            .map(str::to_string);
        for identity in ordered_tls_identity_candidates(&self.identities, server_name.as_deref()) {
            if apply_dynamic_tls_identity(ssl, identity).is_ok() {
                return;
            }

            warn!(
                listener = %self.listener_name,
                secret = %identity.secret_ref,
                sni = server_name.as_deref().unwrap_or_default(),
                "failed to apply dynamic tls identity during handshake, trying next certificate"
            );
        }

        warn!(
            listener = %self.listener_name,
            sni = server_name.as_deref().unwrap_or_default(),
            "failed to apply any configured tls identity during handshake"
        );
    }

    async fn handshake_complete_callback(
        &self,
        ssl: &pingora::tls::ssl::SslRef,
    ) -> Option<Arc<dyn std::any::Any + Send + Sync>> {
        Some(Arc::new(DownstreamTlsInfo {
            server_name: ssl
                .servername(pingora::tls::ssl::NameType::HOST_NAME)
                .map(str::to_string)
                .unwrap_or_default(),
            client_certificate_present: ssl.peer_certificate().is_some(),
        }) as Arc<dyn std::any::Any + Send + Sync>)
    }
}

fn apply_dynamic_tls_identity(
    ssl: &mut pingora::tls::ssl::SslRef,
    identity: &TlsIdentity,
) -> Result<()> {
    let certs =
        X509::stack_from_pem(identity.cert_pem.as_bytes()).context("parse certificate PEM")?;
    let Some(leaf) = certs.first() else {
        return Err(anyhow!("no certificates found in PEM"));
    };
    let key =
        PKey::private_key_from_pem(identity.key_pem.as_bytes()).context("parse private key PEM")?;

    pingora::tls::ext::ssl_use_certificate(ssl, leaf).context("load leaf certificate")?;
    for cert in certs.iter().skip(1) {
        pingora::tls::ext::ssl_add_chain_cert(ssl, cert).context("load certificate chain")?;
    }
    pingora::tls::ext::ssl_use_private_key(ssl, &key).context("load private key")?;
    Ok(())
}

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

    if let Err(err) = add_plain_http_service(
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
    if let Err(err) = add_tls_http_service(
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
fn add_plain_http_service(
    server: &mut Server,
    plan: &RuntimePlan,
    snapshot: SharedSnapshot,
    runtime: &RuntimeOptions,
    access_log: AccessLogOptions,
    session_persistence: SessionPersistenceOptions,
    traffic: SharedTrafficStats,
    admission: HttpAdmissionController,
    circuit_breaker: HttpCircuitBreakerController,
    rate_limit: HttpRateLimitController,
    retry_budget: RetryBudgetController,
) -> Result<()> {
    let plain_listeners: Vec<&RuntimeListener> = plan
        .listeners
        .iter()
        .filter(|listener| matches!(listener.protocol, RuntimeListenerProtocol::Plain))
        .collect();
    if plain_listeners.is_empty() {
        return Ok(());
    }

    let listener_name_hint = listener_name_hint(&plain_listeners);
    let listener_port_hint = listener_port_hint(&plain_listeners);
    let mut service = ProxyServiceBuilder::new(
        &server.configuration,
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
            upstream_tuning_from_runtime(runtime),
            runtime.request_tracing_enabled,
            runtime.max_request_body_bytes,
            runtime.max_request_header_bytes,
            listener_name_hint,
            listener_port_hint,
            runtime.cache.clone(),
            runtime.experimental.clone(),
        ),
    )
    .name("Nantian Gateway HTTP")
    .server_options(plain_http_server_options(runtime.keepalive_request_limit))
    .build();

    let reuse_port = reuse_port_for_runtime(runtime);
    for listener in plain_listeners {
        service.add_tcp_with_settings(
            listener.bind.as_str(),
            tcp_socket_options_for_bind(
                listener.bind.as_str(),
                runtime.downstream_tcp_keepalive.clone(),
                reuse_port,
                runtime.downstream_tcp_fastopen,
                runtime.downstream_dscp,
            ),
        );
    }

    server.add_service(service);
    Ok(())
}

pub(super) fn plain_http_server_options(keepalive_request_limit: Option<u32>) -> HttpServerOptions {
    let mut server_options = HttpServerOptions::default();
    server_options.h2c = true;
    server_options.keepalive_request_limit = keepalive_request_limit;
    server_options
}

#[allow(clippy::too_many_arguments)]
fn add_tls_http_service(
    server: &mut Server,
    plan: &RuntimePlan,
    snapshot: SharedSnapshot,
    runtime: &RuntimeOptions,
    access_log: AccessLogOptions,
    session_persistence: SessionPersistenceOptions,
    traffic: SharedTrafficStats,
    admission: HttpAdmissionController,
    circuit_breaker: HttpCircuitBreakerController,
    rate_limit: HttpRateLimitController,
    retry_budget: RetryBudgetController,
) -> Result<()> {
    let tls_listeners: Vec<&RuntimeListener> = plan
        .listeners
        .iter()
        .filter(|listener| matches!(listener.protocol, RuntimeListenerProtocol::Tls { .. }))
        .collect();
    if tls_listeners.is_empty() {
        return Ok(());
    }

    let listener_name_hint = listener_name_hint(&tls_listeners);
    let listener_port_hint = listener_port_hint(&tls_listeners);
    let mut service = ProxyServiceBuilder::new(
        &server.configuration,
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
            upstream_tuning_from_runtime(runtime),
            runtime.request_tracing_enabled,
            runtime.max_request_body_bytes,
            runtime.max_request_header_bytes,
            listener_name_hint,
            listener_port_hint,
            runtime.cache.clone(),
            runtime.experimental.clone(),
        ),
    )
    .name("Nantian Gateway HTTPS")
    .server_options(tls_http_server_options(runtime.keepalive_request_limit))
    .build();

    let reuse_port = reuse_port_for_runtime(runtime);
    for listener in tls_listeners {
        let RuntimeListenerProtocol::Tls {
            identities,
            client_ca_path,
            frontend_validation_mode,
            ..
        } = &listener.protocol
        else {
            continue;
        };

        let mut settings = TlsSettings::with_callbacks(Box::new(DynamicTlsCertificates {
            listener_name: listener.name.clone(),
            identities: identities.clone(),
        }))?;
        settings.enable_h2();
        settings.set_session_cache_mode(SslSessionCacheMode::SERVER);
        if let Some(client_ca_path) = client_ca_path.as_deref() {
            if let Err(err) = settings.set_ca_file(client_ca_path) {
                warn!(
                    bind = %listener.bind,
                    ca_path = %client_ca_path,
                    error = %err,
                    "skipping tls http listener because the configured client CA bundle is invalid"
                );
                continue;
            }
            if matches!(
                frontend_validation_mode.as_deref(),
                Some("AllowInsecureFallback")
            ) {
                settings.set_verify(SslVerifyMode::PEER);
            } else {
                settings.set_verify(SslVerifyMode::PEER | SslVerifyMode::FAIL_IF_NO_PEER_CERT);
            }
            settings
                .set_session_id_context(b"ntgw-http")
                .context("set TLS session id context")?;
        }
        service.add_tls_with_settings(
            listener.bind.as_str(),
            Some(tcp_socket_options_for_bind(
                listener.bind.as_str(),
                runtime.downstream_tcp_keepalive.clone(),
                reuse_port,
                runtime.downstream_tcp_fastopen,
                runtime.downstream_dscp,
            )),
            settings,
        );
    }

    server.add_service(service);
    Ok(())
}

fn tls_http_server_options(keepalive_request_limit: Option<u32>) -> HttpServerOptions {
    let mut server_options = HttpServerOptions::default();
    server_options.keepalive_request_limit = keepalive_request_limit;
    server_options
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
        build_wasm_filter(&snapshot)
    } else {
        None
    };
    let ai_filter = if experimental.enable_ai_gateway {
        build_ai_filter(&snapshot, wasm_filter.clone())
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

fn build_ai_filter(
    snapshot: &SharedSnapshot,
    wasm_filter: Option<Arc<ntgw_ai::wasm_filter::WasmPluginFilter>>,
) -> Option<Arc<ntgw_ai::filter::AIGatewayFilter>> {
    use ntgw_ai::filter::AIGatewayFilterBuilder;
    use ntgw_ai::format::AdapterRegistry;
    use ntgw_ai::format::anthropic::AnthropicAdapter;
    use ntgw_ai::format::ollama::OllamaAdapter;
    use ntgw_ai::format::openai::OpenAIAdapter;
    use ntgw_ai::observability::metrics::AIMetrics;
    use ntgw_ai::ratelimit::{RateLimitConfig, TokenRateLimiter};

    let registry = prometheus::Registry::new();
    let metrics = match AIMetrics::new(&registry) {
        Ok(m) => Arc::new(m),
        Err(e) => {
            tracing::warn!(
                target: "ai_gateway",
                error = %e,
                "failed to create AI metrics, AI gateway disabled"
            );
            return None;
        }
    };

    let mut adapters = AdapterRegistry::new();
    adapters.register("openai", Arc::new(OpenAIAdapter));
    adapters.register("anthropic", Arc::new(AnthropicAdapter));
    adapters.register("ollama", Arc::new(OllamaAdapter));
    let adapters = Arc::new(adapters);

    let rate_limiter = {
        let snap = snapshot.load();
        snap.backends.iter().find_map(|b| {
            b.token_policy.as_ref().map(|tp| {
                TokenRateLimiter::new(RateLimitConfig {
                    tokens_per_minute: tp.tokens_per_minute,
                    tokens_per_hour: tp.tokens_per_hour,
                    requests_per_minute: tp.requests_per_minute,
                    scope: tp.scope.clone(),
                    burst: tp.burst,
                    on_limit: tp.on_limit.clone(),
                })
            })
        })
    };

    let mut builder = AIGatewayFilterBuilder::new(adapters, metrics);
    if let Some(rl) = rate_limiter {
        builder = builder.rate_limiter(rl);
    }
    if let Some(wf) = wasm_filter {
        builder = builder.wasm_filter(wf);
    }
    // Additional subsystems (langfuse, cost_tracker, pii_masker, prompt_guard,
    // content_safety, model_router, fallback, ab_engine, tenant_manager, ai_sandbox,
    // prompt_injector) are configured per-backend via AIService CRDs and applied
    // at request time through the filter's pre_process/post_process methods.
    // They do not require global wiring at build time.
    Some(Arc::new(builder.build()))
}

fn build_wasm_filter(
    snapshot: &SharedSnapshot,
) -> Option<Arc<ntgw_ai::wasm_filter::WasmPluginFilter>> {
    use ntgw_ai::wasm_filter::WasmPluginFilter;
    use ntgw_wasm::plugin::{WasmHook, WasmPluginSpec, WasmSandboxConfig, global_plugin_manager};

    let snapshot_guard = snapshot.load();
    let mut desired: Vec<WasmPluginSpec> = Vec::new();

    for backend in &snapshot_guard.backends {
        let Some(ref wp) = backend.wasm_plugin else {
            continue;
        };
        if wp.wasm_bytes.is_empty() {
            continue;
        }

        let hooks: Vec<WasmHook> = wp
            .hooks
            .iter()
            .filter_map(|h| {
                serde_json::from_value(serde_json::Value::String(h.clone()))
                    .ok()
                    .or_else(|| {
                        tracing::warn!(
                            target: "wasm",
                            backend = %backend.name,
                            hook = %h,
                            "unknown wasm hook, skipping"
                        );
                        None
                    })
            })
            .collect();

        if hooks.is_empty() {
            tracing::warn!(
                target: "wasm",
                backend = %backend.name,
                "no valid hooks configured for wasm plugin, skipping"
            );
            continue;
        }

        let config: serde_json::Value =
            serde_json::from_str(&wp.config_json).unwrap_or(serde_json::Value::Null);

        let sandbox = WasmSandboxConfig {
            max_memory_bytes: {
                let mb = wp.sandbox.max_memory_bytes;
                if mb > usize::MAX as u64 {
                    usize::MAX
                } else {
                    mb as usize
                }
            },
            max_execution_ms: wp.sandbox.max_execution_time_ms,
        };

        desired.push((
            wp.name.clone(),
            wp.wasm_bytes.clone(),
            config,
            hooks,
            sandbox,
            if wp.sha256.is_empty() {
                None
            } else {
                Some(wp.sha256.clone())
            },
        ));
    }

    drop(snapshot_guard);

    let pm = match global_plugin_manager() {
        Ok(pm) => pm,
        Err(error) => {
            tracing::warn!(
                target: "wasm",
                error = %error,
                "failed to initialize wasm plugin manager"
            );
            return None;
        }
    };

    if desired.is_empty() {
        for name in pm.plugin_names() {
            pm.unload_plugin(&name);
        }
        return None;
    }

    let (loaded, updated, skipped, unloaded) = pm.diff_and_apply(&desired);
    tracing::info!(
        target: "wasm",
        loaded,
        updated,
        skipped,
        unloaded,
        "applied wasm plugin snapshot"
    );

    let plugin_names = pm.plugin_names();
    if plugin_names.is_empty() {
        return None;
    }

    Some(Arc::new(WasmPluginFilter::new(pm, plugin_names)))
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
