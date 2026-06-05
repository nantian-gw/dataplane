use std::sync::Arc;

use aeg_http::{
    build_http_app, AccessLogOptions, RuntimeOptions as HttpRuntimeOptions,
    SessionPersistenceOptions,
};
use aeg_observability::{
    HttpCircuitBreakerController, HttpRateLimitController, RetryBudgetController,
    SharedTrafficStats,
};
use anyhow::Result;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    sync::watch,
};

use crate::{listener_plan::build_listener_plan, runtime::ConnectionConfig, RuntimeOptions};

#[tokio::test]
async fn passthrough_dispatch_replays_preread_bytes_to_upstream() -> Result<()> {
    let upstream_listener = TcpListener::bind("127.0.0.1:0").await?;
    let upstream_addr = upstream_listener.local_addr()?;
    let gateway_port = super::free_tcp_port();
    let snapshot =
        super::shared_tls_snapshot(gateway_port, super::free_tcp_port(), upstream_addr.port());
    let plan = build_listener_plan(&snapshot.read(), &RuntimeOptions::default())?;
    let bind = Arc::new(
        plan.binds
            .get(&format!("127.0.0.1:{gateway_port}"))
            .cloned()
            .expect("bind"),
    );
    let gateway_listener = TcpListener::bind(("127.0.0.1", gateway_port)).await?;
    let app = build_http_app(
        snapshot.clone(),
        HttpRuntimeOptions::default(),
        AccessLogOptions {
            enabled: false,
            ..AccessLogOptions::default()
        },
        SessionPersistenceOptions::build(None, None)?,
        SharedTrafficStats::shared(),
        aeg_observability::OverloadStats::shared(),
        HttpCircuitBreakerController::new(Default::default()),
        HttpRateLimitController::new(Default::default()),
        RetryBudgetController::new(Default::default()),
        None,
    )?;
    let (_shutdown_tx, shutdown_rx) = watch::channel(false);
    let observed_hello = super::build_client_hello("passthrough.example.com");
    let expected_len = observed_hello.len();

    let upstream = tokio::spawn(async move {
        let (mut stream, _) = upstream_listener.accept().await?;
        let mut observed = vec![0; expected_len];
        stream.read_exact(&mut observed).await?;
        Ok::<Vec<u8>, anyhow::Error>(observed)
    });

    let server = tokio::spawn(async move {
        let (stream, _) = gateway_listener.accept().await?;
        crate::runtime::handle_connection(
            bind,
            stream,
            snapshot,
            app,
            shutdown_rx,
            ConnectionConfig,
        )
        .await
    });

    let mut client = TcpStream::connect(("127.0.0.1", gateway_port)).await?;
    client.write_all(&observed_hello).await?;
    client.shutdown().await?;

    let observed = upstream.await??;
    server.await??;

    assert!(observed.starts_with(&[0x16, 0x03]));
    assert_eq!(
        observed,
        super::build_client_hello("passthrough.example.com")
    );
    Ok(())
}
