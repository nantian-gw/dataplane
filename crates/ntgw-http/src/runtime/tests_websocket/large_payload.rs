#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn websocket_large_payload_tunnels_in_both_directions() {
    install_rustls_provider();
    let upstream_listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("upstream bind");
    let upstream_addr = upstream_listener.local_addr().expect("upstream addr");
    let gateway_port = free_tcp_port();
    let snapshot = simple_http_snapshot(gateway_port, "/chat", upstream_addr.port() as u32, "HTTP");
    let runtime = RuntimeOptions {
        enable_ipv6: false,
        ..RuntimeOptions::default()
    };
    let plan = build_listener_plan(&snapshot.load(), &runtime, None).expect("plan");
    let server = start_server(
        plan,
        snapshot.clone(),
        runtime,
        AccessLogOptions {
            enabled: false,
            ..AccessLogOptions::default()
        },
        SessionPersistenceOptions::build(None, None).expect("session options"),
        SharedTrafficStats::shared(),
    )
    .expect("start server");

    let payload = vec![b'x'; 256 * 1024];
    let upstream_payload = payload.clone();
    let client_payload = payload.clone();
    let upstream = tokio::spawn(async move {
        let (mut stream, _) = upstream_listener.accept().await?;
        let request = read_http_headers(&mut stream).await?;
        assert!(request.starts_with("GET /chat HTTP/1.1\r\n"));
        stream
            .write_all(
                b"HTTP/1.1 101 Switching Protocols\r\nConnection: Upgrade\r\nUpgrade: websocket\r\n\r\n",
            )
            .await?;

        let mut received = vec![0; upstream_payload.len()];
        stream.read_exact(&mut received).await?;
        assert_eq!(received, upstream_payload);

        stream.write_all(&received).await?;
        Ok::<(), anyhow::Error>(())
    });

    wait_for_listener(gateway_port).await;

    let result = async {
        let mut client = TcpStream::connect(("127.0.0.1", gateway_port)).await?;
        client
            .write_all(
                b"GET /chat HTTP/1.1\r\nHost: example.com\r\nConnection: Upgrade\r\nUpgrade: websocket\r\n\r\n",
            )
            .await?;
        let response = read_http_headers(&mut client).await?;
        assert!(response.starts_with("HTTP/1.1 101"));

        client.write_all(&client_payload).await?;
        let mut echoed = vec![0; client_payload.len()];
        client.read_exact(&mut echoed).await?;
        assert_eq!(echoed, client_payload);
        // Let the proxy finish flushing its write buffers before we stop the server.
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        Ok::<(), anyhow::Error>(())
    }
    .await;

    stop_server(server);
    result.expect("websocket client flow");
    upstream
        .await
        .expect("upstream task")
        .expect("upstream result");
}
