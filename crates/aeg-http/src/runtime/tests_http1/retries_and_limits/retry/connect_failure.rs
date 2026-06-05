#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn retry_after_connect_failure_reselects_next_backend_and_records_retry_metrics() {
    install_rustls_provider();
    let healthy_listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("healthy backend bind");
    let healthy_addr = healthy_listener.local_addr().expect("healthy addr");
    let failing_port = free_tcp_port();
    let gateway_port = free_tcp_port();
    let retry = RetryPolicy {
        codes: vec![503],
        attempts: 2,
        backoff: None,
    };
    let snapshot = multi_backend_http_snapshot(
        gateway_port,
        "/retry-connect",
        &[
            ("failing", failing_port as u32, "HTTP", 1),
            ("healthy", healthy_addr.port() as u32, "HTTP", 1),
        ],
        Some(retry),
    );
    let runtime = RuntimeOptions {
        enable_ipv6: false,
        ..RuntimeOptions::default()
    };
    let plan = build_listener_plan(&snapshot.read(), &runtime, None).expect("plan");
    let traffic = SharedTrafficStats::shared();
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

    let healthy = tokio::spawn(async move {
        let (mut stream, _) = healthy_listener.accept().await?;
        let request = read_http_headers(&mut stream).await?;
        assert!(request.starts_with("GET /retry-connect HTTP/1.1\r\n"));
        stream
            .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nok")
            .await?;
        Ok::<(), anyhow::Error>(())
    });

    wait_for_listener(gateway_port).await;

    let result = async {
        let mut client = TcpStream::connect(("127.0.0.1", gateway_port)).await?;
        client
            .write_all(b"GET /retry-connect HTTP/1.1\r\nHost: example.com\r\n\r\n")
            .await?;
        let response = read_http_response(&mut client).await?;
        assert!(response.starts_with("HTTP/1.1 200"));
        assert!(response.ends_with("\r\n\r\nok"));
        Ok::<(), anyhow::Error>(())
    }
    .await;

    stop_server(server);
    result.expect("retry connect client flow");
    healthy
        .await
        .expect("healthy task")
        .expect("healthy result");

    let stats = wait_for_traffic_snapshot(&traffic, |stats| {
        stats.total_events == 1 && stats.total_retry_attempts == 1 && stats.status_2xx == 1
    })
    .await;
    assert_eq!(stats.total_events, 1);
    assert_eq!(stats.total_retried_events, 1);
    assert_eq!(stats.total_retry_attempts, 1);
    assert_eq!(stats.status_2xx, 1);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn default_transport_retry_after_connect_failure_reselects_fast_path_backend() {
    install_rustls_provider();
    let healthy_listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("healthy backend bind");
    let healthy_addr = healthy_listener.local_addr().expect("healthy addr");
    let stale_port = free_tcp_port();
    let gateway_port = free_tcp_port();
    let snapshot = multi_backend_http_snapshot(
        gateway_port,
        "/retry-connect-default",
        &[
            ("stale", stale_port as u32, "HTTP", 1),
            ("healthy", healthy_addr.port() as u32, "HTTP", 1),
        ],
        None,
    );
    let runtime = RuntimeOptions {
        enable_ipv6: false,
        ..RuntimeOptions::default()
    };
    let plan = build_listener_plan(&snapshot.read(), &runtime, None).expect("plan");
    let traffic = SharedTrafficStats::shared();
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

    let healthy = tokio::spawn(async move {
        let (mut stream, _) = healthy_listener.accept().await?;
        let request = read_http_headers(&mut stream).await?;
        assert!(request.starts_with(
            "GET /retry-connect-default HTTP/1.1\r\n"
        ));
        stream
            .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nok")
            .await?;
        Ok::<(), anyhow::Error>(())
    });

    wait_for_listener(gateway_port).await;

    let result = async {
        let mut client = TcpStream::connect(("127.0.0.1", gateway_port)).await?;
        client
            .write_all(b"GET /retry-connect-default HTTP/1.1\r\nHost: example.com\r\n\r\n")
            .await?;
        let response = read_http_response(&mut client).await?;
        assert!(
            response.starts_with("HTTP/1.1 200"),
            "transport connect failure should retry to healthy backend, got: {response}"
        );
        assert!(response.ends_with("\r\n\r\nok"));
        Ok::<(), anyhow::Error>(())
    }
    .await;

    stop_server(server);
    result.expect("default transport retry client flow");
    healthy
        .await
        .expect("healthy task")
        .expect("healthy result");

    let stats = wait_for_traffic_snapshot(&traffic, |stats| {
        stats.total_events == 1 && stats.total_retry_attempts == 1 && stats.status_2xx == 1
    })
    .await;
    assert_eq!(stats.total_events, 1);
    assert_eq!(stats.total_retried_events, 1);
    assert_eq!(stats.total_retry_attempts, 1);
    assert_eq!(stats.status_2xx, 1);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn default_transport_retry_avoids_failed_endpoint_for_concurrent_fast_path_requests() {
    install_rustls_provider();
    let healthy_listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("healthy backend bind");
    let healthy_addr = healthy_listener.local_addr().expect("healthy addr");
    let stale_port = free_tcp_port();
    let gateway_port = free_tcp_port();
    let request_count = 8usize;
    let snapshot = multi_backend_http_snapshot(
        gateway_port,
        "/retry-connect-concurrent",
        &[
            ("stale", stale_port as u32, "HTTP", 1),
            ("healthy", healthy_addr.port() as u32, "HTTP", 1),
        ],
        None,
    );
    let runtime = RuntimeOptions {
        enable_ipv6: false,
        ..RuntimeOptions::default()
    };
    let plan = build_listener_plan(&snapshot.read(), &runtime, None).expect("plan");
    let traffic = SharedTrafficStats::shared();
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

    let accepted = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let healthy_accepted = accepted.clone();
    let (healthy_shutdown_tx, mut healthy_shutdown_rx) = oneshot::channel::<()>();
    let healthy = tokio::spawn(async move {
        loop {
            tokio::select! {
                _ = &mut healthy_shutdown_rx => break,
                accepted = healthy_listener.accept() => {
                    let (mut stream, _) = accepted?;
                    healthy_accepted.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                    tokio::spawn(async move {
                        let request = read_http_headers(&mut stream).await?;
                        assert!(request.starts_with(
                            "GET /retry-connect-concurrent HTTP/1.1\r\n"
                        ));
                        stream
                            .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nok")
                            .await?;
                        Ok::<(), anyhow::Error>(())
                    });
                }
            }
        }
        Ok::<(), anyhow::Error>(())
    });

    wait_for_listener(gateway_port).await;

    let mut clients = Vec::with_capacity(request_count);
    for _ in 0..request_count {
        clients.push(tokio::spawn(async move {
            let mut client = TcpStream::connect(("127.0.0.1", gateway_port)).await?;
            client
                .write_all(
                    b"GET /retry-connect-concurrent HTTP/1.1\r\nHost: example.com\r\n\r\n",
                )
                .await?;
            read_http_response(&mut client).await
        }));
    }

    let mut unexpected = Vec::new();
    for client in clients {
        let response = client.await.expect("client task").expect("client flow");
        if !response.starts_with("HTTP/1.1 200") {
            unexpected.push(response);
        }
    }

    stop_server(server);
    let _ = healthy_shutdown_tx.send(());
    healthy.await.expect("healthy task").expect("healthy result");

    assert!(
        unexpected.is_empty(),
        "transport retry should not select a just-failed endpoint again, got: {unexpected:#?}"
    );
    assert!(
        accepted.load(std::sync::atomic::Ordering::SeqCst) >= request_count,
        "healthy backend should receive all original healthy selections and successful retries"
    );

    let stats = wait_for_traffic_snapshot(&traffic, |stats| {
        stats.total_events == request_count as u64 && stats.status_5xx == 0
    })
    .await;
    assert_eq!(stats.total_events, request_count as u64);
    assert_eq!(stats.status_5xx, 0);
}
