use super::*;
use std::sync::Arc;

use pingora::{server::configuration::ServerConf, tls::ssl::SslSessionCacheMode};

use crate::proxy::DownstreamTlsInfo;

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

#[allow(clippy::too_many_arguments)]
pub(super) fn add_tls_http_service(
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
