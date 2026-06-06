#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn retry_budget_exhaustion_blocks_extra_retry_attempts() {
    install_rustls_provider();
    let failing_listener_a = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("failing backend a bind");
    let failing_addr_a = failing_listener_a.local_addr().expect("failing addr a");
    let failing_listener_b = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("failing backend b bind");
    let failing_addr_b = failing_listener_b.local_addr().expect("failing addr b");
    let healthy_listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("healthy backend bind");
    let healthy_addr = healthy_listener.local_addr().expect("healthy addr");
    let gateway_port = free_tcp_port();
    let traffic = SharedTrafficStats::shared();
    let snapshot = multi_backend_http_snapshot(
        gateway_port,
        "/retry-budget",
        &[
            ("failing-a", failing_addr_a.port() as u32, "HTTP", 1),
            ("failing-b", failing_addr_b.port() as u32, "HTTP", 1),
            // Keep the first two selections on failing backends so this test
            // isolates retry-budget exhaustion instead of weighted ordering.
            ("healthy", healthy_addr.port() as u32, "HTTP", 2),
        ],
        Some(RetryPolicy {
            codes: vec![503],
            attempts: 3,
            backoff: None,
        }),
    );
    let runtime = RuntimeOptions {
        enable_ipv6: false,
        retry_budget: ntgw_observability::RetryBudgetOptions {
            enabled: true,
            ratio_percent: 0,
            burst: 1,
        },
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
        traffic.clone(),
    )
    .expect("start server");

    let failing_a = tokio::spawn(async move {
        let (mut stream, _) = failing_listener_a.accept().await?;
        let request = read_http_headers(&mut stream).await?;
        assert!(request.starts_with("GET /retry-budget HTTP/1.1\r\n"));
        stream
            .write_all(b"HTTP/1.1 503 Service Unavailable\r\nContent-Length: 6\r\n\r\nfail-a")
            .await?;
        Ok::<(), anyhow::Error>(())
    });
    let failing_b = tokio::spawn(async move {
        let (mut stream, _) = failing_listener_b.accept().await?;
        let request = read_http_headers(&mut stream).await?;
        assert!(request.starts_with("GET /retry-budget HTTP/1.1\r\n"));
        stream
            .write_all(b"HTTP/1.1 503 Service Unavailable\r\nContent-Length: 6\r\n\r\nfail-b")
            .await?;
        Ok::<(), anyhow::Error>(())
    });

    wait_for_listener(gateway_port).await;

    let result = async {
        let mut client = TcpStream::connect(("127.0.0.1", gateway_port)).await?;
        client
            .write_all(b"GET /retry-budget HTTP/1.1\r\nHost: example.com\r\n\r\n")
            .await?;
        let response = read_http_response(&mut client).await?;
        assert!(
            response.starts_with("HTTP/1.1 502"),
            "expected retry budget exhaustion to stop before the healthy backend, got: {response}"
        );
        Ok::<(), anyhow::Error>(())
    }
    .await;

    stop_server(server);
    result.expect("retry budget client flow");
    failing_a
        .await
        .expect("failing a task")
        .expect("failing a result");
    failing_b
        .await
        .expect("failing b task")
        .expect("failing b result");

    let healthy_unreached = timeout(Duration::from_millis(300), healthy_listener.accept()).await;
    assert!(
        healthy_unreached.is_err(),
        "healthy backend should not be reached once the retry budget is exhausted"
    );

    let stats = wait_for_traffic_snapshot(&traffic, |stats| {
        stats.total_events == 1 && stats.total_retry_attempts == 1 && stats.status_5xx == 1
    })
    .await;
    assert_eq!(stats.total_events, 1);
    assert_eq!(stats.total_retry_attempts, 1);
    assert_eq!(stats.total_retried_events, 1);
    assert_eq!(stats.status_5xx, 1);
}
