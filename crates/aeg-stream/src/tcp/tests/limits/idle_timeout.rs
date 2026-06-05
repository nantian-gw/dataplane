#[tokio::test]
async fn plain_tcp_session_idle_timeout_closes_stalled_connection() -> Result<()> {
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
        let mut buf = [0; 4];
        stream.read_exact(&mut buf).await?;
        assert_eq!(&buf, b"ping");
        stream.write_all(b"pong").await?;
        let mut eof = [0; 1];
        let read = timeout(Duration::from_millis(400), stream.read(&mut eof)).await??;
        assert_eq!(read, 0, "upstream should observe idle timeout closure");
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
            Some(Duration::from_millis(75)),
            None,
            std::sync::Arc::new(crate::pool::TcpConnectionPool::new(0, Duration::from_secs(30))),
        )
        .await
    });

    let mut client = TcpStream::connect(gateway_addr).await?;
    client.write_all(b"ping").await?;
    let mut response = [0; 4];
    client.read_exact(&mut response).await?;
    assert_eq!(&response, b"pong");
    let mut eof = [0; 1];
    let read = timeout(Duration::from_millis(400), client.read(&mut eof)).await??;
    assert_eq!(read, 0, "client should observe idle timeout closure");

    upstream.await??;
    server.await??;
    Ok(())
}
