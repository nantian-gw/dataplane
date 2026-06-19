#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn websocket_backend_close_propagates_to_client() {
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

    let upstream = tokio::spawn(async move {
        let (mut stream, _) = upstream_listener.accept().await?;
        let _ = read_http_headers(&mut stream).await?;
        stream
            .write_all(
                b"HTTP/1.1 101 Switching Protocols\r\nConnection: Upgrade\r\nUpgrade: websocket\r\n\r\nbye",
            )
            .await?;
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

        let mut payload = [0; 3];
        client.read_exact(&mut payload).await?;
        assert_eq!(&payload, b"bye");

        let mut eof = [0; 1];
        let read = client.read(&mut eof).await?;
        assert_eq!(read, 0);
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

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn websocket_client_close_propagates_to_backend() {
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

    let upstream = tokio::spawn(async move {
        let (mut stream, _) = upstream_listener.accept().await?;
        let _ = read_http_headers(&mut stream).await?;
        stream
            .write_all(
                b"HTTP/1.1 101 Switching Protocols\r\nConnection: Upgrade\r\nUpgrade: websocket\r\n\r\n",
            )
            .await?;

        let mut eof = [0; 1];
        let read = stream.read(&mut eof).await?;
        assert_eq!(read, 0);
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
        client.shutdown().await?;
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
