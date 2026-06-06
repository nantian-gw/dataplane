#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn http1_listener_inflight_budget_fast_fails_unmatched_second_request() {
    install_rustls_provider();
    let upstream_listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("upstream bind");
    let upstream_addr = upstream_listener.local_addr().expect("upstream addr");
    let gateway_port = free_tcp_port();
    let snapshot = simple_http_snapshot(
        gateway_port,
        "/limited",
        upstream_addr.port() as u32,
        "HTTP",
    );
    let overload = ntgw_observability::OverloadStats::shared();
    let runtime = RuntimeOptions {
        enable_ipv6: false,
        admission: ntgw_observability::HttpAdmissionOptions {
            listener_inflight_limit: 1,
            ..ntgw_observability::HttpAdmissionOptions::default()
        },
        ..RuntimeOptions::default()
    };
    let plan = build_listener_plan(&snapshot.read(), &runtime, None).expect("plan");
    let server = start_server_with_overload_stats(
        plan,
        snapshot.clone(),
        runtime,
        AccessLogOptions {
            enabled: false,
            ..AccessLogOptions::default()
        },
        SessionPersistenceOptions::build(None, None).expect("session options"),
        SharedTrafficStats::shared(),
        overload.clone(),
    )
    .expect("start server");

    let (accepted_tx, accepted_rx) = tokio::sync::oneshot::channel();
    let (release_tx, release_rx) = tokio::sync::oneshot::channel();
    let upstream = tokio::spawn(async move {
        let (mut stream, _) = upstream_listener.accept().await?;
        let request = read_http_headers(&mut stream).await?;
        assert!(request.starts_with("GET /limited HTTP/1.1\r\n"));
        let _ = accepted_tx.send(());
        let _ = release_rx.await;
        stream
            .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nok")
            .await?;
        Ok::<(), anyhow::Error>(())
    });

    wait_for_listener(gateway_port).await;

    let first = tokio::spawn(async move {
        let mut client = TcpStream::connect(("127.0.0.1", gateway_port)).await?;
        client
            .write_all(b"GET /limited HTTP/1.1\r\nHost: example.com\r\n\r\n")
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
        .write_all(b"GET /missing HTTP/1.1\r\nHost: example.com\r\n\r\n")
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
    upstream
        .await
        .expect("upstream task")
        .expect("upstream result");

    let overload_snapshot = overload.snapshot();
    assert_eq!(overload_snapshot.http_rejected_total, 1);
    assert_eq!(overload_snapshot.http_rejected_listener_total, 1);
}
