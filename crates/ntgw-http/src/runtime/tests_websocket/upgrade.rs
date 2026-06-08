#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn websocket_upgrade_successfully_tunnels_over_http_backend() {
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
    let plan = build_listener_plan(&snapshot.read(), &runtime, None).expect("plan");
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
        let request = read_http_headers(&mut stream).await?;
        assert!(request.starts_with("GET /chat HTTP/1.1\r\n"));
        assert!(request
            .to_ascii_lowercase()
            .contains("upgrade: websocket\r\n"));
        assert!(request
            .to_ascii_lowercase()
            .contains("connection: upgrade\r\n"));

        stream
            .write_all(
                b"HTTP/1.1 101 Switching Protocols\r\nConnection: Upgrade\r\nUpgrade: websocket\r\n\r\n",
            )
            .await?;

        let mut payload = [0; 5];
        stream.read_exact(&mut payload).await?;
        assert_eq!(&payload, b"hello");
        stream.write_all(b"world").await?;
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
        assert!(response
            .to_ascii_lowercase()
            .contains("upgrade: websocket\r\n"));
        client.write_all(b"hello").await?;
        let mut echoed = [0; 5];
        client.read_exact(&mut echoed).await?;
        assert_eq!(&echoed, b"world");
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
