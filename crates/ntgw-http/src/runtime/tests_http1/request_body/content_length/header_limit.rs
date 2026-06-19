#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn http1_request_header_limit_rejects_large_headers_before_proxying() {
    install_rustls_provider();
    let upstream_listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("upstream bind");
    let upstream_addr = upstream_listener.local_addr().expect("upstream addr");
    let gateway_port = free_tcp_port();
    let snapshot = simple_http_snapshot(
        gateway_port,
        "/headers-limited",
        upstream_addr.port() as u32,
        "HTTP",
    );
    let runtime = RuntimeOptions {
        enable_ipv6: false,
        max_request_header_bytes: 16,
        ..RuntimeOptions::default()
    };
    let plan = build_listener_plan(&snapshot.load(), &runtime, None).expect("plan");
    let log_path = temp_log_path("header-limit-access-log");
    let server = start_server(
        plan,
        snapshot.clone(),
        runtime,
        AccessLogOptions {
            enabled: true,
            path: log_path.display().to_string(),
            mode: ntgw_observability::AccessLogMode::Text,
            format: "$status|$sent_http_server|$sent_http_cache_control|$sent_http_content_length"
                .to_string(),
            ..AccessLogOptions::default()
        },
        SessionPersistenceOptions::build(None, None).expect("session options"),
        SharedTrafficStats::shared(),
    )
    .expect("start server");

    wait_for_listener(gateway_port).await;

    let result = async {
        let mut client = TcpStream::connect(("127.0.0.1", gateway_port)).await?;
        client
            .write_all(
                b"GET /headers-limited HTTP/1.1\r\nHost: example.com\r\nX-Large: abcdefghijklmnop\r\n\r\n",
            )
            .await?;
        let response = read_http_response(&mut client).await?;
        assert!(
            response.starts_with("HTTP/1.1 431"),
            "expected 431 response, got: {response}"
        );
        Ok::<(), anyhow::Error>(())
    }
    .await;

    stop_server(server);
    result.expect("oversized request-header client flow");

    let no_upstream = timeout(Duration::from_millis(300), upstream_listener.accept()).await;
    assert!(
        no_upstream.is_err(),
        "oversized request headers should not reach upstream"
    );

    let log_contents = wait_for_log_contents(&log_path).await;
    let line = log_contents
        .lines()
        .find(|line| line.starts_with("431|"))
        .expect("431 access-log line");
    let parts: Vec<_> = line.split('|').collect();
    assert_eq!(parts.len(), 4, "expected four pipe-separated fields");
    assert_ne!(parts[1], "-", "server header should be captured");
    assert_eq!(parts[2], "private, no-store");
    assert_eq!(parts[3], "0");

    shutdown_access_log_writer(&log_path.display().to_string());
    let _ = fs::remove_file(log_path);
}
