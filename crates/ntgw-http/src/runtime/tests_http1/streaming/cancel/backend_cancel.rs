#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn streaming_http_backend_cancel_does_not_retry_after_response_started() {
    install_rustls_provider();
    let streaming_listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("streaming backend bind");
    let streaming_addr = streaming_listener.local_addr().expect("streaming addr");
    let retry_listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("retry backend bind");
    let retry_addr = retry_listener.local_addr().expect("retry addr");
    let gateway_port = free_tcp_port();
    let snapshot = multi_backend_http_snapshot(
        gateway_port,
        "/api/aggregate/streamable-http",
        &[
            ("streaming", streaming_addr.port() as u32, "HTTP", 1),
            ("retry-target", retry_addr.port() as u32, "HTTP", 1),
        ],
        Some(RetryPolicy {
            codes: vec![500, 503, 504],
            attempts: 2,
            backoff: None,
        }),
    );
    let runtime = RuntimeOptions {
        enable_ipv6: false,
        ..RuntimeOptions::default()
    };
    let plan = build_listener_plan(&snapshot.read(), &runtime, None).expect("plan");
    let log_path = temp_log_path("streaming-http-backend-cancel");
    let traffic = SharedTrafficStats::shared();
    let server = start_server(
        plan,
        snapshot.clone(),
        runtime,
        AccessLogOptions {
            enabled: true,
            path: log_path.display().to_string(),
            ..AccessLogOptions::default()
        },
        SessionPersistenceOptions::build(None, None).expect("session options"),
        traffic.clone(),
    )
    .expect("start server");

    let streaming = tokio::spawn(async move {
        let (mut stream, _) = streaming_listener.accept().await?;
        let request = read_http_headers(&mut stream).await?;
        assert!(request.starts_with("GET /api/aggregate/streamable-http HTTP/1.1\r\n"));
        stream
            .write_all(
                b"HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n\r\n",
            )
            .await?;
        write_h1_chunk(&mut stream, b"event: open\n\n").await?;
        stream.flush().await?;
        Ok::<(), anyhow::Error>(())
    });

    wait_for_listener(gateway_port).await;

    let result = async {
        let mut client = TcpStream::connect(("127.0.0.1", gateway_port)).await?;
        client
            .write_all(
                b"GET /api/aggregate/streamable-http HTTP/1.1\r\nHost: k8s-mcp.kubernetes.com\r\nAccept: text/event-stream\r\n\r\n",
            )
            .await?;
        let response_headers = read_http_headers(&mut client).await?;
        assert!(
            response_headers.starts_with("HTTP/1.1 200"),
            "expected initial streaming response to stay 200, got: {response_headers}"
        );
        let first_event = read_next_h1_chunk(&mut client).await?;
        assert_eq!(first_event, b"event: open\n\n");
        let _ = read_all_with_timeout(&mut client).await?;
        Ok::<(), anyhow::Error>(())
    }
    .await;

    result.expect("streaming backend cancel flow");
    streaming
        .await
        .expect("streaming backend task")
        .expect("streaming backend result");

    let retry_unreached = timeout(Duration::from_millis(500), retry_listener.accept()).await;
    stop_server(server);
    assert!(
        retry_unreached.is_err(),
        "backend-cancelled stream with started response must not be retried to the next backend"
    );

    let stats = wait_for_traffic_snapshot(&traffic, |stats| stats.total_events == 1).await;
    assert_eq!(stats.total_events, 1);
    assert_eq!(stats.status_2xx, 1);
    assert_eq!(stats.total_retry_attempts, 0);
    assert_eq!(stats.total_retried_events, 0);

    let log_contents = wait_for_log_contents(&log_path).await;
    assert!(log_contents.contains("\"status\":200"));
    assert!(log_contents.contains("\"retryAttempts\":0"));
    assert!(
        log_contents.contains("\"responseFlags\":\"UC\""),
        "backend cancel should be logged as upstream close, got: {log_contents}"
    );

    shutdown_access_log_writer(&log_path.display().to_string());
    let _ = fs::remove_file(log_path);
}
