use super::*;

pub(super) fn add_plain_http_service(
    server: &mut Server,
    plan: &RuntimePlan,
    opts: &GatewayProxyOptions,
    runtime: &RuntimeOptions,
) -> Result<()> {
    let plain_listeners: Vec<&RuntimeListener> = plan
        .listeners
        .iter()
        .filter(|listener| matches!(listener.protocol, RuntimeListenerProtocol::Plain))
        .collect();
    if plain_listeners.is_empty() {
        return Ok(());
    }

    let mut service = ProxyServiceBuilder::new(
        &server.configuration,
        build_gateway_proxy(opts, runtime),
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