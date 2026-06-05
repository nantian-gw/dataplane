#[tokio::test]
async fn proxies_plain_tcp_sessions() -> Result<()> {
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

#[tokio::test]
async fn proxies_plain_tcp_sessions_emit_runtime_ids_in_access_log() -> Result<()> {
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
    rebuild_runtime_indexes(&snapshot);
    let expected_runtime_ids = selected_runtime_ids(&snapshot, &listener.name);
    let log_path = temp_log_path("tcp-runtime-ids");

    let upstream = tokio::spawn(async move {
        let (mut stream, _) = upstream_listener.accept().await?;
        let mut buf = [0; 4];
        stream.read_exact(&mut buf).await?;
        assert_eq!(&buf, b"ping");
        stream.write_all(b"pong").await?;
        stream.shutdown().await?;
        Ok::<(), anyhow::Error>(())
    });

    let access_log = json_access_log(&log_path);
    let server = tokio::spawn(async move {
        let (stream, _) = gateway_listener.accept().await?;
        handle_connection(
            snapshot,
            listener.name,
            stream,
            false,
            access_log,
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

    let contents = wait_for_log_contents(&log_path, "\"event\":\"tcp_session\"").await?;
    assert_runtime_id_fields(&contents, expected_runtime_ids);
    cleanup_access_log(&log_path);
    Ok(())
}

#[tokio::test]
async fn downstream_reset_records_tcp_session_without_task_error() -> Result<()> {
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
    let traffic = SharedTrafficStats::shared();
    let observed_traffic = traffic.clone();
    let (upstream_received_tx, upstream_received_rx) = tokio::sync::oneshot::channel();

    let upstream = tokio::spawn(async move {
        let (mut stream, _) = upstream_listener.accept().await?;
        let mut buf = [0; 4];
        stream.read_exact(&mut buf).await?;
        assert_eq!(&buf, b"ping");
        let _ = upstream_received_tx.send(());

        let mut eof = [0; 1];
        let read = timeout(Duration::from_millis(500), stream.read(&mut eof)).await??;
        assert_eq!(read, 0, "upstream should observe gateway-side shutdown");
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
            traffic,
            TCP_PROXY_BUFFER_BYTES,
            None,
            None,
            std::sync::Arc::new(crate::pool::TcpConnectionPool::new(0, Duration::from_secs(30))),
        )
        .await
    });

    let mut client = TcpStream::connect(gateway_addr).await?;
    client.write_all(b"ping").await?;
    upstream_received_rx
        .await
        .expect("upstream should receive payload before client reset");
    #[allow(deprecated)]
    client.set_linger(Some(Duration::from_secs(0)))?;
    drop(client);

    upstream.await??;
    server
        .await
        .expect("server task should join")
        .expect("downstream reset should be recorded as TCP traffic, not task failure");

    let stats = observed_traffic.snapshot();
    assert_eq!(stats.total_events, 1);
    assert_eq!(stats.total_request_events, 0);
    assert!(
        stats.response_flags.is_empty(),
        "TCP transport flags must not pollute request response flag counters: {:?}",
        stats.response_flags
    );
    Ok(())
}
