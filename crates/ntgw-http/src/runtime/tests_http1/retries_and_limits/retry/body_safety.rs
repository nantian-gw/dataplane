#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn retryable_response_status_does_not_retry_post_body_to_next_backend() {
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
    let traffic = SharedTrafficStats::shared();
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
        traffic.clone(),
    )
    .expect("start server");

    let failing = tokio::spawn(async move {
        let (mut stream, _) = failing_listener.accept().await?;
        let request = read_http_headers(&mut stream).await?;
        assert!(request.starts_with("POST /retry-status HTTP/1.1\r\n"));
        assert_eq!(
            header_value(&request, "content-length").as_deref(),
            Some("11")
        );
        let body = read_http_body(&mut stream, &request).await?;
        assert_eq!(body, b"hello world");
        stream
            .write_all(b"HTTP/1.1 503 Service Unavailable\r\nContent-Length: 4\r\n\r\nfail")
            .await?;
        Ok::<(), anyhow::Error>(())
    });
    let healthy = tokio::spawn(async move {
        let accepted = timeout(Duration::from_millis(400), healthy_listener.accept()).await;
        let stream = match accepted {
            Ok(Ok((stream, _))) => stream,
            Ok(Err(err)) => return Err(err.into()),
            Err(_) => return Ok(false),
        };
        let mut stream = stream;
        let request = read_http_headers(&mut stream).await?;
        assert!(request.starts_with("POST /retry-status HTTP/1.1\r\n"));
        stream
            .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nok")
            .await?;
        Ok::<bool, anyhow::Error>(true)
    });

    wait_for_listener(gateway_port).await;

    let result = async {
        let mut client = TcpStream::connect(("127.0.0.1", gateway_port)).await?;
        client
            .write_all(
                b"POST /retry-status HTTP/1.1\r\nHost: example.com\r\nContent-Length: 11\r\n\r\nhello world",
            )
            .await?;
        let response = read_http_response(&mut client).await?;
        assert!(
            response.starts_with("HTTP/1.1 503"),
            "non-replayable POST body should receive the original upstream status, got: {response}"
        );
        assert!(response.ends_with("\r\n\r\nfail"));
        Ok::<(), anyhow::Error>(())
    }
    .await;

    stop_server(server);
    result.expect("non-replayable post body client flow");
    failing
        .await
        .expect("failing task")
        .expect("failing result");
    let healthy_reached = healthy
        .await
        .expect("healthy task")
        .expect("healthy result");
    assert!(
        !healthy_reached,
        "POST request body must not be replayed to the next backend"
    );

    let stats = wait_for_traffic_snapshot(&traffic, |stats| {
        stats.total_events == 1 && stats.status_5xx == 1
    })
    .await;
    assert_eq!(stats.total_retry_attempts, 0);
    assert_eq!(stats.total_retried_events, 0);
}
