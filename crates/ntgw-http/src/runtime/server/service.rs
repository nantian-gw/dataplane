use super::*;

#[allow(clippy::too_many_arguments)]
pub(super) fn add_plain_http_service(
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

pub(crate) fn plain_http_server_options(keepalive_request_limit: Option<u32>) -> HttpServerOptions {
    let mut server_options = HttpServerOptions::default();
    server_options.h2c = true;
    server_options.keepalive_request_limit = keepalive_request_limit;
    server_options
}
