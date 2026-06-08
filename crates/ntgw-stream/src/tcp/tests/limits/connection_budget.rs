#[tokio::test]
async fn tcp_listener_connection_budget_rejects_second_session() -> Result<()> {
    let upstream_listener = TcpListener::bind("127.0.0.1:0").await?;
    let upstream_addr = upstream_listener.local_addr()?;
    let gateway_listener = TcpListener::bind("127.0.0.1:0").await?;
    let gateway_addr = gateway_listener.local_addr()?;
    let listener = test_listener(
        "default/gw/tcp",
        gateway_addr.port() as u32,
        "LISTENER_PROTOCOL_TCP",
    );
    let snapshot = test_snapshot(
        listener.clone(),
        "tcp-route",
        "ROUTE_KIND_TCP",
        vec![ntgw_ir::StreamMatch {
            port: gateway_addr.port() as u32,
            sni_hostname: String::new(),
            mode: ntgw_ir::TlsRouteMode::default(),
        }],
        upstream_addr,
    );
    let overload = ntgw_observability::OverloadStats::shared();
    let (shutdown_tx, shutdown_rx) = watch::channel(false);
    let server = tokio::spawn(run_with_listener(
        snapshot,
        listener.name.clone(),
        gateway_addr.to_string(),
        gateway_listener,
        shutdown_rx,
        false,
        disabled_access_log(),
        SharedTrafficStats::shared(),
        ntgw_observability::TcpAdmissionController::new(
            ntgw_observability::TcpAdmissionOptions {
                listener_connection_limit: 1,
                ..ntgw_observability::TcpAdmissionOptions::default()
            },
            overload.clone(),
        ),
        TCP_PROXY_BUFFER_BYTES,
        None,
        None,
            std::sync::Arc::new(crate::pool::TcpConnectionPool::new(0, Duration::from_secs(30))),
    ));

    let (accepted_tx, accepted_rx) = tokio::sync::oneshot::channel();
    let (release_tx, release_rx) = tokio::sync::oneshot::channel();
    let upstream = tokio::spawn(async move {
        let (mut stream, _) = upstream_listener.accept().await?;
        let mut buf = [0; 4];
        stream.read_exact(&mut buf).await?;
        assert_eq!(&buf, b"ping");
        let _ = accepted_tx.send(());
        let _ = release_rx.await;
        stream.write_all(b"pong").await?;
        stream.shutdown().await?;
        Ok::<(), anyhow::Error>(())
    });

    let mut first = TcpStream::connect(gateway_addr).await?;
    first.write_all(b"ping").await?;
    first.shutdown().await?;
    accepted_rx
        .await
        .expect("first session should reach upstream");

    let mut second = TcpStream::connect(gateway_addr).await?;
    second.write_all(b"next").await?;
    let mut second_buf = [0; 1];
    let second_read = timeout(Duration::from_millis(200), second.read(&mut second_buf)).await??;
    assert_eq!(
        second_read, 0,
        "second session should be closed immediately"
    );

    release_tx.send(()).expect("release first session");
    let mut response = Vec::new();
    first.read_to_end(&mut response).await?;
    assert_eq!(response, b"pong");

    shutdown_tx.send(true).expect("shutdown stream listener");
    upstream.await??;
    server.await??;

    let overload_snapshot = overload.snapshot();
    assert_eq!(overload_snapshot.tcp_rejected_total, 1);
    assert_eq!(overload_snapshot.tcp_rejected_listener_total, 1);
    Ok(())
}
