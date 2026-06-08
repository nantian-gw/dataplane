#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn streaming_http_response_allows_idle_period_before_first_body_chunk() {
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
    let runtime = RuntimeOptions {
        enable_ipv6: false,
        ..RuntimeOptions::default()
    };
    let plan = build_listener_plan(&snapshot.read(), &runtime, None).expect("plan");
    let log_path = temp_log_path("streaming-http-idle-first-body");
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
        stream.flush().await?;
        sleep(Duration::from_millis(350)).await;
        stream.write_all(b"0\r\n\r\n").await?;
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
        let response = read_http_response(&mut client).await?;
        let response_lower = response.to_ascii_lowercase();
        assert!(
            response.starts_with("HTTP/1.1 200"),
            "expected idle streaming response to stay 200, got: {response}"
        );
        assert!(
            response_lower.contains("content-type: text/event-stream\r\n"),
            "expected SSE content type, got: {response}"
        );
        assert!(
            response.ends_with("\r\n\r\n"),
            "expected headers-only stream to finish cleanly, got: {response}"
        );
        Ok::<(), anyhow::Error>(())
    }
    .await;

    stop_server(server);
    result.expect("headers-only idle streaming client flow");
    upstream
        .await
        .expect("upstream task")
        .expect("upstream result");

    let stats = wait_for_traffic_snapshot(&traffic, |stats| {
        stats.total_events == 1 && stats.status_2xx == 1
    })
    .await;
    assert_eq!(stats.total_events, 1);
    assert_eq!(stats.status_2xx, 1);
    assert_eq!(stats.status_5xx, 0);
    assert_eq!(stats.total_retry_attempts, 0);

    let log_contents = wait_for_log_contents(&log_path).await;
    assert!(log_contents.contains("\"status\":200"));
    assert!(log_contents.contains("\"retryAttempts\":0"));
    assert!(log_contents.contains("\"responseFlags\":\"\""));
    assert!(
        !log_contents.contains("\"responseFlags\":\"UT\""),
        "headers-only idle stream should not be logged as upstream timeout: {log_contents}"
    );

    shutdown_access_log_writer(&log_path.display().to_string());
    let _ = fs::remove_file(log_path);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn streaming_http_zero_backend_policy_timeout_waits_for_response_headers() {
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
        let mut current = snapshot.write();
        current.backend_policies.insert(
            format!("default/backend:{}", upstream_addr.port()),
            BackendPolicy {
                connect_timeout: Some(Duration::from_secs(5)),
                request_timeout: Some(Duration::ZERO),
                ..BackendPolicy::default()
            },
        );
    }
    let runtime = RuntimeOptions {
        enable_ipv6: false,
        ..RuntimeOptions::default()
    };
    let plan = build_listener_plan(&snapshot.read(), &runtime, None).expect("plan");
    let log_path = temp_log_path("streaming-http-zero-backend-policy-timeout");
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
        sleep(Duration::from_millis(50)).await;
        stream
            .write_all(
                b"HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n\r\n",
            )
            .await?;
        write_h1_chunk(&mut stream, b"event: open\n\n").await?;
        stream.write_all(b"0\r\n\r\n").await?;
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
        let response = read_http_response(&mut client).await?;
        assert!(
            response.starts_with("HTTP/1.1 200"),
            "zero backend policy timeout must not become immediate 504, got: {response}"
        );
        assert!(
            response.ends_with("\r\n\r\nevent: open\n\n"),
            "expected streamed event after delayed response headers, got: {response}"
        );
        Ok::<(), anyhow::Error>(())
    }
    .await;

    stop_server(server);
    result.expect("zero backend policy timeout streaming client flow");
    upstream
        .await
        .expect("upstream task")
        .expect("upstream result");

    let stats = wait_for_traffic_snapshot(&traffic, |stats| stats.total_events == 1).await;
    assert_eq!(stats.total_events, 1);
    assert_eq!(stats.status_2xx, 1);
    assert_eq!(stats.status_5xx, 0);

    let log_contents = wait_for_log_contents(&log_path).await;
    assert!(log_contents.contains("\"status\":200"));
    assert!(log_contents.contains("\"responseFlags\":\"\""));
    assert!(
        !log_contents.contains("\"responseFlags\":\"UT\""),
        "zero backend policy timeout should not be logged as upstream timeout: {log_contents}"
    );

    shutdown_access_log_writer(&log_path.display().to_string());
    let _ = fs::remove_file(log_path);
}
