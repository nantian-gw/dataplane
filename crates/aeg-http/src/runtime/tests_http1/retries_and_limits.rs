include!("retries_and_limits/retry.rs");

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn route_rate_limit_fast_fails_second_request_with_429() {
    install_rustls_provider();
    let upstream_listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("upstream bind");
    let upstream_addr = upstream_listener.local_addr().expect("upstream addr");
    let gateway_port = free_tcp_port();
    let snapshot = simple_http_snapshot(
        gateway_port,
        "/ratelimited",
        upstream_addr.port() as u32,
        "HTTP",
    );
    let runtime = RuntimeOptions {
        enable_ipv6: false,
        rate_limit: aeg_observability::HttpRateLimitOptions {
            route_requests_per_second: 1,
            route_burst: 1,
            ..aeg_observability::HttpRateLimitOptions::default()
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
        SharedTrafficStats::shared(),
    )
    .expect("start server");

    let upstream = tokio::spawn(async move {
        let (mut stream, _) = upstream_listener.accept().await?;
        let request = read_http_headers(&mut stream).await?;
        assert!(request.starts_with("GET /ratelimited HTTP/1.1\r\n"));
        stream
            .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nok")
            .await?;
        let no_second_upstream =
            timeout(Duration::from_millis(300), upstream_listener.accept()).await;
        Ok::<bool, anyhow::Error>(no_second_upstream.is_err())
    });

    wait_for_listener(gateway_port).await;

    let result = async {
        let mut first = TcpStream::connect(("127.0.0.1", gateway_port)).await?;
        first
            .write_all(b"GET /ratelimited HTTP/1.1\r\nHost: example.com\r\n\r\n")
            .await?;
        let first_response = read_http_response(&mut first).await?;
        assert!(
            first_response.starts_with("HTTP/1.1 200"),
            "unexpected first response: {first_response}"
        );

        let mut second = TcpStream::connect(("127.0.0.1", gateway_port)).await?;
        second
            .write_all(b"GET /ratelimited HTTP/1.1\r\nHost: example.com\r\n\r\n")
            .await?;
        let second_response = read_http_response(&mut second).await?;
        assert!(
            second_response.starts_with("HTTP/1.1 429"),
            "expected 429 response, got: {second_response}"
        );
        Ok::<(), anyhow::Error>(())
    }
    .await;

    stop_server(server);
    result.expect("rate limit client flow");
    let no_second_upstream = upstream
        .await
        .expect("upstream task")
        .expect("upstream result");
    assert!(
        no_second_upstream,
        "second request should be rejected locally by the route rate limit"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn backend_circuit_breaker_fast_fails_second_request_with_503() {
    install_rustls_provider();
    let upstream_listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("upstream bind");
    let upstream_addr = upstream_listener.local_addr().expect("upstream addr");
    let gateway_port = free_tcp_port();
    let snapshot = simple_http_snapshot(
        gateway_port,
        "/breaker",
        upstream_addr.port() as u32,
        "HTTP",
    );
    let runtime = RuntimeOptions {
        enable_ipv6: false,
        circuit_breaker: aeg_observability::HttpCircuitBreakerOptions {
            backend_max_inflight_requests: 1,
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
        SharedTrafficStats::shared(),
    )
    .expect("start server");

    let (accepted_tx, accepted_rx) = tokio::sync::oneshot::channel();
    let (release_tx, release_rx) = tokio::sync::oneshot::channel();
    let upstream = tokio::spawn(async move {
        let (mut stream, _) = upstream_listener.accept().await?;
        let request = read_http_headers(&mut stream).await?;
        assert!(request.starts_with("GET /breaker HTTP/1.1\r\n"));
        let _ = accepted_tx.send(());
        let _ = release_rx.await;
        stream
            .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nok")
            .await?;
        let no_second_upstream =
            timeout(Duration::from_millis(300), upstream_listener.accept()).await;
        Ok::<bool, anyhow::Error>(no_second_upstream.is_err())
    });

    wait_for_listener(gateway_port).await;

    let first = tokio::spawn(async move {
        let mut client = TcpStream::connect(("127.0.0.1", gateway_port)).await?;
        client
            .write_all(b"GET /breaker HTTP/1.1\r\nHost: example.com\r\n\r\n")
            .await?;
        read_http_response(&mut client).await
    });

    accepted_rx
        .await
        .expect("first request should reach upstream");

    let mut second = TcpStream::connect(("127.0.0.1", gateway_port))
        .await
        .expect("second client connect");
    second
        .write_all(b"GET /breaker HTTP/1.1\r\nHost: example.com\r\n\r\n")
        .await
        .expect("second request write");
    let second_response = read_http_response(&mut second)
        .await
        .expect("second response");
    assert!(
        second_response.starts_with("HTTP/1.1 503"),
        "expected 503 response, got: {second_response}"
    );

    release_tx
        .send(())
        .expect("release first upstream response");
    let first_response = first
        .await
        .expect("first task join")
        .expect("first response");
    assert!(first_response.starts_with("HTTP/1.1 200"));

    stop_server(server);
    let no_second_upstream = upstream
        .await
        .expect("upstream task")
        .expect("upstream result");
    assert!(
        no_second_upstream,
        "second request should be rejected locally by the backend circuit breaker"
    );
}
