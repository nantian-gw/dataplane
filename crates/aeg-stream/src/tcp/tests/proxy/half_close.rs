#[tokio::test]
async fn plain_tcp_proxy_preserves_client_half_close_until_upstream_reply() -> Result<()> {
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
        vec![aeg_ir::StreamMatch {
            port: gateway_addr.port() as u32,
            sni_hostname: String::new(),
            mode: aeg_ir::TlsRouteMode::default(),
        }],
        upstream_addr,
    );

    let upstream = tokio::spawn(async move {
        let (mut stream, _) = upstream_listener.accept().await?;
        let mut request = Vec::new();
        stream.read_to_end(&mut request).await?;
        assert_eq!(request, b"ping");
        stream.write_all(b"pong").await?;
        stream.shutdown().await?;
        Ok::<(), anyhow::Error>(())
    });

    let server = tokio::spawn(async move {
        let (stream, _) = gateway_listener.accept().await?;
        handle_connection(
            snapshot,
            listener.name,
            stream,
            false,
            disabled_access_log(),
            SharedTrafficStats::shared(),
            TCP_PROXY_BUFFER_BYTES,
            None,
            None,
            std::sync::Arc::new(crate::pool::TcpConnectionPool::new(0, Duration::from_secs(30))),
        )
        .await
    });

    let mut client = TcpStream::connect(gateway_addr).await?;
    client.write_all(b"ping").await?;
    client.shutdown().await?;
    let mut response = Vec::new();
    client.read_to_end(&mut response).await?;

    assert_eq!(response, b"pong");
    upstream.await??;
    server.await??;
    Ok(())
}
