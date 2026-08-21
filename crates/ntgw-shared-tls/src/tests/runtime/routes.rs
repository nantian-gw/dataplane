use super::*;
use ntgw_http::cache::{CacheManager, CacheOptions};
use ntgw_http::proxy::GatewayProxyOptions;
use ntgw_observability::{HttpAdmissionController, HttpAdmissionOptions, OverloadStats};
use std::sync::Arc;
use std::time::Duration;

use std::{collections::BTreeMap, pin::Pin};

use anyhow::Context;
use ntgw_ir::{
    BackendCluster, BackendEndpoint, BackendRef, SharedSnapshot, StreamMatch, StreamRoute,
    StreamRule, TlsRouteMode,
};
use pingora::{
    protocols::tls::SslStream,
    tls::ssl::{SslConnector, SslMethod, SslVerifyMode},
};

use super::super::example_secret_material;

#[tokio::test]
async fn shared_tls_runtime_routes_passthrough_and_terminate_on_same_bind() -> Result<()> {
    let stream_backend = TcpListener::bind("127.0.0.1:0").await?;
    let stream_backend_addr = stream_backend.local_addr()?;
    let http_backend = TcpListener::bind("127.0.0.1:0").await?;
    let http_backend_addr = http_backend.local_addr()?;
    let gateway_port = free_tcp_port();
    let bind_addr = format!("127.0.0.1:{gateway_port}");
    let snapshot = shared_tls_snapshot(
        gateway_port,
        http_backend_addr.port(),
        stream_backend_addr.port(),
    );
    let plan = build_listener_plan(&snapshot.load(), &RuntimeOptions::default())?;
    let bind = Arc::new(plan.binds.get(&bind_addr).cloned().expect("bind"));
    let gateway_listener = TcpListener::bind(&bind_addr).await?;
    let opts = GatewayProxyOptions {
        snapshot: snapshot.clone(),
        access_log: AccessLogOptions {
            enabled: false,
            ..AccessLogOptions::default()
        },
        session_persistence: SessionPersistenceOptions::build(None, None)?,
        traffic: SharedTrafficStats::shared(),
        admission: HttpAdmissionController::new(
            HttpAdmissionOptions::default(),
            OverloadStats::shared(),
        ),
        circuit_breaker: HttpCircuitBreakerController::new(Default::default()),
        rate_limit: HttpRateLimitController::new(Default::default()),
        retry_budget: RetryBudgetController::new(Default::default()),
        downstream_read_timeout: None,
        downstream_max_connection_age: None,
        upstream_tcp_keepalive: None,
        upstream_tuning: Default::default(),
        request_tracing_enabled: false,
        max_request_body_bytes: 0,
        max_request_header_bytes: 0,
        ai_gateway_max_request_body_bytes: 0,
        listener_name_hint: None,
        listener_port_hint: None,
        cache: CacheManager::new(CacheOptions {
            enabled: false,
            max_size_bytes: 0,
            max_entry_size_bytes: 0,
            default_ttl: Duration::from_secs(0),
        }),
        wasm_filter: None,
        ai_filter: None,
    };
    let app = build_http_app(opts, HttpRuntimeOptions::default())?;
    let (_shutdown_tx, shutdown_rx) = watch::channel(false);

    let upstream_stream = tokio::spawn(async move {
        let (mut stream, _) = stream_backend.accept().await?;
        let mut observed = vec![0; build_client_hello("passthrough.example.com").len()];
        use tokio::io::AsyncReadExt;
        stream.read_exact(&mut observed).await?;
        stream.write_all(b"stream-backend").await?;
        stream.shutdown().await?;
        Ok::<(), anyhow::Error>(())
    });
    let upstream_http = tokio::spawn(async move {
        let (mut stream, _) = http_backend.accept().await?;
        let request = read_http_headers(&mut stream).await?;
        assert!(request.starts_with("GET / HTTP/1.1\r\n"));
        stream
            .write_all(
                b"HTTP/1.1 200 OK\r\nContent-Length: 12\r\nConnection: close\r\n\r\nhttp-backend",
            )
            .await?;
        Ok::<(), anyhow::Error>(())
    });

    let server = tokio::spawn(async move {
        loop {
            let (stream, _) = gateway_listener.accept().await?;
            let bind = bind.clone();
            let snapshot = snapshot.clone();
            let app = app.clone();
            let shutdown_rx = shutdown_rx.clone();
            tokio::spawn(async move {
                let _ = crate::runtime::handle_connection(
                    bind,
                    stream,
                    snapshot,
                    app,
                    shutdown_rx,
                    ConnectionConfig,
                )
                .await;
            });
        }
        #[allow(unreachable_code)]
        Ok::<(), anyhow::Error>(())
    });

    let mut passthrough = TcpStream::connect(&bind_addr).await?;
    passthrough
        .write_all(&build_client_hello("passthrough.example.com"))
        .await?;
    passthrough.shutdown().await?;
    let mut passthrough_response = Vec::new();
    use tokio::io::AsyncReadExt;
    passthrough.read_to_end(&mut passthrough_response).await?;
    assert_eq!(String::from_utf8(passthrough_response)?, "stream-backend");

    let terminated = https_request(&bind_addr, "server-san.example", "example.com", "/").await?;
    assert!(terminated.contains("200 OK"));
    assert!(terminated.ends_with("\r\n\r\nhttp-backend"));

    upstream_stream.await??;
    upstream_http.await??;
    server.abort();
    Ok(())
}

#[tokio::test]
async fn shared_tls_runtime_terminates_tlsroute_to_raw_tcp_backend_on_same_bind() -> Result<()> {
    let terminated_backend = TcpListener::bind("127.0.0.1:0").await?;
    let terminated_backend_addr = terminated_backend.local_addr()?;
    let passthrough_backend = TcpListener::bind("127.0.0.1:0").await?;
    let passthrough_backend_addr = passthrough_backend.local_addr()?;
    let gateway_port = free_tcp_port();
    let bind_addr = format!("127.0.0.1:{gateway_port}");
    let snapshot = tlsroute_terminate_snapshot(
        gateway_port,
        terminated_backend_addr.port(),
        passthrough_backend_addr.port(),
    );
    let plan = build_listener_plan(&snapshot.load(), &RuntimeOptions::default())?;
    let bind = Arc::new(plan.binds.get(&bind_addr).cloned().expect("bind"));
    let gateway_listener = TcpListener::bind(&bind_addr).await?;
    let opts = GatewayProxyOptions {
        snapshot: snapshot.clone(),
        access_log: AccessLogOptions {
            enabled: false,
            ..AccessLogOptions::default()
        },
        session_persistence: SessionPersistenceOptions::build(None, None)?,
        traffic: SharedTrafficStats::shared(),
        admission: HttpAdmissionController::new(
            HttpAdmissionOptions::default(),
            OverloadStats::shared(),
        ),
        circuit_breaker: HttpCircuitBreakerController::new(Default::default()),
        rate_limit: HttpRateLimitController::new(Default::default()),
        retry_budget: RetryBudgetController::new(Default::default()),
        downstream_read_timeout: None,
        downstream_max_connection_age: None,
        upstream_tcp_keepalive: None,
        upstream_tuning: Default::default(),
        request_tracing_enabled: false,
        max_request_body_bytes: 0,
        max_request_header_bytes: 0,
        ai_gateway_max_request_body_bytes: 0,
        listener_name_hint: None,
        listener_port_hint: None,
        cache: CacheManager::new(CacheOptions {
            enabled: false,
            max_size_bytes: 0,
            max_entry_size_bytes: 0,
            default_ttl: Duration::from_secs(0),
        }),
        wasm_filter: None,
        ai_filter: None,
    };
    let app = build_http_app(opts, HttpRuntimeOptions::default())?;
    let (_shutdown_tx, shutdown_rx) = watch::channel(false);

    let upstream_terminated = tokio::spawn(async move {
        let (mut stream, _) = terminated_backend.accept().await?;
        let mut request = [0; 5];
        use tokio::io::AsyncReadExt;
        stream.read_exact(&mut request).await?;
        assert_eq!(&request, b"PING\n");
        stream.write_all(b"raw-terminated").await?;
        stream.shutdown().await?;
        Ok::<(), anyhow::Error>(())
    });

    let server = tokio::spawn(async move {
        loop {
            let (stream, _) = gateway_listener.accept().await?;
            let bind = bind.clone();
            let snapshot = snapshot.clone();
            let app = app.clone();
            let shutdown_rx = shutdown_rx.clone();
            tokio::spawn(async move {
                let _ = crate::runtime::handle_connection(
                    bind,
                    stream,
                    snapshot,
                    app,
                    shutdown_rx,
                    ConnectionConfig,
                )
                .await;
            });
        }
        #[allow(unreachable_code)]
        Ok::<(), anyhow::Error>(())
    });

    let mut terminated = tls_client_stream(&bind_addr, "tls.example.com").await?;
    terminated.write_all(b"PING\n").await?;
    let mut response = Vec::new();
    use tokio::io::AsyncReadExt;
    timeout(
        Duration::from_secs(2),
        terminated.read_to_end(&mut response),
    )
    .await??;
    assert_eq!(String::from_utf8(response)?, "raw-terminated");

    upstream_terminated.await??;
    server.abort();
    Ok(())
}

async fn tls_client_stream(bind: &str, sni: &str) -> Result<SslStream<TcpStream>> {
    let tcp = TcpStream::connect(bind).await?;
    let mut connector = SslConnector::builder(SslMethod::tls()).context("ssl connector")?;
    connector.set_verify(SslVerifyMode::NONE);
    let ssl = connector
        .build()
        .configure()
        .context("ssl configure")?
        .into_ssl(sni)
        .context("ssl create")?;
    let mut stream = SslStream::new(ssl, tcp).context("ssl stream")?;
    Pin::new(&mut stream)
        .connect()
        .await
        .context("ssl connect")?;
    Ok(stream)
}

fn tlsroute_terminate_snapshot(
    gateway_port: u16,
    terminated_backend_port: u16,
    passthrough_backend_port: u16,
) -> SharedSnapshot {
    let shared = Snapshot::shared();
    shared.store(Arc::new(Snapshot {
        listeners: vec![
            Listener {
                name: "default/gw/tls-terminate".to_string(),
                address: "127.0.0.1".to_string(),
                addresses: vec!["127.0.0.1".to_string()],
                port: gateway_port as u32,
                protocol: "LISTENER_PROTOCOL_TLS".to_string(),
                hostnames: vec!["tls.example.com".to_string()],
                attached_routes: vec!["default/terminated-route".to_string()],
                tls: Some(TlsConfig {
                    enabled: true,
                    passthrough: false,
                    secret_refs: vec!["default/example-cert".to_string()],
                    sni_hosts: vec![],
                    min_version: "1.2".to_string(),
                    max_version: "1.3".to_string(),
                    frontend_validation: None,
                }),
                ..Listener::default()
            },
            Listener {
                name: "default/gw/tls-passthrough".to_string(),
                address: "127.0.0.1".to_string(),
                addresses: vec!["127.0.0.1".to_string()],
                port: gateway_port as u32,
                protocol: "LISTENER_PROTOCOL_TLS_PASSTHROUGH".to_string(),
                hostnames: vec!["passthrough.example.com".to_string()],
                attached_routes: vec!["default/passthrough-route".to_string()],
                tls: Some(TlsConfig {
                    enabled: true,
                    passthrough: true,
                    secret_refs: vec![],
                    sni_hosts: vec![],
                    min_version: String::new(),
                    max_version: String::new(),
                    frontend_validation: None,
                }),
                ..Listener::default()
            },
        ],
        stream_routes: vec![
            StreamRoute {
                name: "terminated-route".to_string(),
                namespace: "default".to_string(),
                kind: "ROUTE_KIND_TLS".to_string(),
                parent_refs: Vec::new(),
                rules: vec![StreamRule {
                    name: String::new(),
                    matches: vec![StreamMatch {
                        port: gateway_port as u32,
                        sni_hostname: "tls.example.com".to_string(),
                        mode: TlsRouteMode::Terminate,
                    }],
                    backend_refs: vec![BackendRef {
                        namespace: "default".to_string(),
                        name: "terminated-backend".to_string(),
                        port: terminated_backend_port as u32,
                        ..BackendRef::default()
                    }],
                }],
                labels: BTreeMap::new(),
                annotations: BTreeMap::new(),
            },
            StreamRoute {
                name: "passthrough-route".to_string(),
                namespace: "default".to_string(),
                kind: "ROUTE_KIND_TLS".to_string(),
                parent_refs: Vec::new(),
                rules: vec![StreamRule {
                    name: String::new(),
                    matches: vec![StreamMatch {
                        port: gateway_port as u32,
                        sni_hostname: "passthrough.example.com".to_string(),
                        mode: TlsRouteMode::Passthrough,
                    }],
                    backend_refs: vec![BackendRef {
                        namespace: "default".to_string(),
                        name: "passthrough-backend".to_string(),
                        port: passthrough_backend_port as u32,
                        ..BackendRef::default()
                    }],
                }],
                labels: BTreeMap::new(),
                annotations: BTreeMap::new(),
            },
        ],
        backends: vec![
            BackendCluster {
                ai_service: None,
                token_policy: None,
                name: format!("terminated-backend:{terminated_backend_port}").into(),
                namespace: "default".to_string().into(),
                protocol: "TCP".to_string().into(),
                endpoints: vec![BackendEndpoint {
                    address: "127.0.0.1".to_string(),
                    port: terminated_backend_port as u32,
                    healthy: true,
                }],
                wasm_plugin: None,

                circuit_breaker: None,
                security_policy: None,
            },
            BackendCluster {
                ai_service: None,
                token_policy: None,
                name: format!("passthrough-backend:{passthrough_backend_port}").into(),
                namespace: "default".to_string().into(),
                protocol: "TCP".to_string().into(),
                endpoints: vec![BackendEndpoint {
                    address: "127.0.0.1".to_string(),
                    port: passthrough_backend_port as u32,
                    healthy: true,
                }],
                wasm_plugin: None,

                circuit_breaker: None,
                security_policy: None,
            },
        ],
        secrets: vec![example_secret_material("example-cert")],
        ..Snapshot::default()
    }));
    let mut s = (**shared.load()).clone();
    s.rebuild_runtime_indexes();
    shared.store(Arc::new(s));
    shared
}
