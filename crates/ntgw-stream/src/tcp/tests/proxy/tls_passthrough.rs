#[tokio::test]
async fn tls_passthrough_selects_backend_by_sni_and_forwards_preface() -> Result<()> {
    let hello = build_client_hello("api.example.com");
    let upstream_listener = TcpListener::bind("127.0.0.1:0").await?;
    let upstream_addr = upstream_listener.local_addr()?;
    let gateway_listener = TcpListener::bind("127.0.0.1:0").await?;
    let gateway_addr = gateway_listener.local_addr()?;
    let listener = test_listener(
        "default/gw/tls",
        gateway_addr.port() as u32,
        "LISTENER_PROTOCOL_TLS_PASSTHROUGH",
    );
    let snapshot = test_snapshot(
        listener.clone(),
        "tls-route",
        "ROUTE_KIND_TLS",
        vec![ntgw_ir::StreamMatch {
            port: gateway_addr.port() as u32,
            sni_hostname: "*.example.com".to_string(),
            mode: ntgw_ir::TlsRouteMode::default(),
        }],
        upstream_addr,
    );

    let expected_hello = hello.clone();
    let upstream = tokio::spawn(async move {
        let (mut stream, _) = upstream_listener.accept().await?;
        let mut buf = vec![0; expected_hello.len()];
        stream.read_exact(&mut buf).await?;
        assert_eq!(buf, expected_hello);
        stream.write_all(b"tls-ok").await?;
        stream.shutdown().await?;
        Ok::<(), anyhow::Error>(())
    });

    let server = tokio::spawn(async move {
        let (stream, _) = gateway_listener.accept().await?;
        handle_connection(
            snapshot,
            listener.name,
            stream,
            true,
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
    client.write_all(&hello).await?;
    client.shutdown().await?;
    let mut response = Vec::new();
    client.read_to_end(&mut response).await?;

    assert_eq!(response, b"tls-ok");
    upstream.await??;
    server.await??;
    Ok(())
}
