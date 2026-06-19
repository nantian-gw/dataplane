#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn streaming_http_explicit_backend_request_timeout_records_ut_without_retry() {
    install_rustls_provider();
    let upstream_listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("upstream bind");
    let upstream_addr = upstream_listener.local_addr().expect("upstream addr");
    let gateway_port = free_tcp_port();
    let snapshot = simple_http_snapshot(
        gateway_port,
        "/api/aggregate/streamable-http",
        upstream_addr.port() as u32,
        "HTTP",
    );
    {
        let mut current = (**snapshot.load()).clone();
        current.http_routes[0].rules[0].timeouts = Some(ntgw_ir::RouteTimeouts {
            request: None,
            backend_request: Some(Duration::from_millis(75)),
        });
        current.rebuild_runtime_indexes();
        snapshot.store(Arc::new(current));
    }
    let runtime = RuntimeOptions {
        enable_ipv6: false,
        ..RuntimeOptions::default()
    };
    let plan = build_listener_plan(&snapshot.load(), &runtime, None).expect("plan");
    let log_path = temp_log_path("streaming-http-explicit-backend-timeout");
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

    let upstream = tokio::spawn(async move {
        let (mut stream, _) = upstream_listener.accept().await?;
        let request = read_http_headers(&mut stream).await?;
        assert!(request.starts_with("GET /api/aggregate/streamable-http HTTP/1.1\r\n"));
        stream
            .write_all(
                b"HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n\r\n",
            )
            .await?;
        write_h1_chunk(&mut stream, b"event: open\n\n").await?;
        stream.flush().await?;
        sleep(Duration::from_millis(250)).await;
        let _ = write_h1_chunk(&mut stream, b"data: too-late\n\n").await;
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
        let rest = read_all_with_timeout(&mut client).await?;
        assert!(
            !String::from_utf8_lossy(&rest).contains("data: too-late"),
            "explicit backend request timeout should stop the late stream chunk: {rest:?}"
        );
        Ok::<(), anyhow::Error>(())
    }
    .await;

    result.expect("explicit backend request timeout flow");
    upstream
        .await
        .expect("upstream task")
        .expect("upstream result");

    stop_server(server);
    let stats = wait_for_traffic_snapshot(&traffic, |stats| stats.total_events == 1).await;
    assert_eq!(stats.total_events, 1);
    assert_eq!(stats.status_2xx, 1);
    assert_eq!(stats.total_retry_attempts, 0);
    assert_eq!(stats.total_retried_events, 0);
    assert_eq!(stats.response_flags.get("UT").copied(), Some(1));

    let log_contents = wait_for_log_contents(&log_path).await;
    assert!(log_contents.contains("\"status\":200"));
    assert!(log_contents.contains("\"retryAttempts\":0"));
    assert!(
        log_contents.contains("\"responseFlags\":\"UT\""),
        "explicit backend request timeout should be logged as UT, got: {log_contents}"
    );

    shutdown_access_log_writer(&log_path.display().to_string());
    let _ = fs::remove_file(log_path);
}
