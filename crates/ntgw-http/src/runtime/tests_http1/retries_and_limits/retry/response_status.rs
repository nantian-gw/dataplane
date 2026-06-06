#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn retryable_response_status_reselects_next_backend() {
    install_rustls_provider();
    let failing_listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("failing backend bind");
    let failing_addr = failing_listener.local_addr().expect("failing addr");
    let healthy_listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("healthy backend bind");
    let healthy_addr = healthy_listener.local_addr().expect("healthy addr");
    let gateway_port = free_tcp_port();
    let snapshot = multi_backend_http_snapshot(
        gateway_port,
        "/retry-status",
        &[
            ("failing", failing_addr.port() as u32, "HTTP", 1),
            ("healthy", healthy_addr.port() as u32, "HTTP", 1),
        ],
        Some(RetryPolicy {
            codes: vec![503],
            attempts: 2,
            backoff: None,
        }),
    );
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

    let failing = tokio::spawn(async move {
        let (mut stream, _) = failing_listener.accept().await?;
        let request = read_http_headers(&mut stream).await?;
        assert!(request.starts_with("GET /retry-status HTTP/1.1\r\n"));
        stream
            .write_all(b"HTTP/1.1 503 Service Unavailable\r\nContent-Length: 4\r\n\r\nfail")
            .await?;
        Ok::<(), anyhow::Error>(())
    });
    let healthy = tokio::spawn(async move {
        let (mut stream, _) = healthy_listener.accept().await?;
        let request = read_http_headers(&mut stream).await?;
        assert!(request.starts_with("GET /retry-status HTTP/1.1\r\n"));
        stream
            .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nok")
            .await?;
        Ok::<(), anyhow::Error>(())
    });

    wait_for_listener(gateway_port).await;

    let result = async {
        let mut client = TcpStream::connect(("127.0.0.1", gateway_port)).await?;
        client
            .write_all(b"GET /retry-status HTTP/1.1\r\nHost: example.com\r\n\r\n")
            .await?;
        let response = read_http_response(&mut client).await?;
        assert!(response.starts_with("HTTP/1.1 200"));
        assert!(response.ends_with("\r\n\r\nok"));
        Ok::<(), anyhow::Error>(())
    }
    .await;

    stop_server(server);
    result.expect("retry status client flow");
    failing
        .await
        .expect("failing task")
        .expect("failing result");
    healthy
        .await
        .expect("healthy task")
        .expect("healthy result");
}
