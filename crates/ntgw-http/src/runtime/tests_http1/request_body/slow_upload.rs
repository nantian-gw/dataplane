#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn slow_http1_request_body_upload_is_forwarded_intact() {
    install_rustls_provider();
    let upstream_listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("upstream bind");
    let upstream_addr = upstream_listener.local_addr().expect("upstream addr");
    let gateway_port = free_tcp_port();
    let snapshot = simple_http_snapshot(
        gateway_port,
        "/upload-slow",
        upstream_addr.port() as u32,
        "HTTP",
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

    let upstream = tokio::spawn(async move {
        let (mut stream, _) = upstream_listener.accept().await?;
        let request = read_http_headers(&mut stream).await?;
        assert!(request.starts_with("POST /upload-slow HTTP/1.1\r\n"));
        assert_eq!(
            header_value(&request, "content-length").as_deref(),
            Some("11")
        );
        let body = read_http_body(&mut stream, &request).await?;
        assert_eq!(body, b"hello world");
        stream
            .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 4\r\n\r\nslow")
            .await?;
        Ok::<(), anyhow::Error>(())
    });

    wait_for_listener(gateway_port).await;

    let result = async {
        let mut client = TcpStream::connect(("127.0.0.1", gateway_port)).await?;
        client
            .write_all(
                b"POST /upload-slow HTTP/1.1\r\nHost: example.com\r\nContent-Length: 11\r\n\r\nhello",
            )
            .await?;
        sleep(Duration::from_millis(150)).await;
        client.write_all(b" world").await?;
        client.flush().await?;
        let response = read_http_response(&mut client).await?;
        assert!(response.starts_with("HTTP/1.1 200"), "unexpected response: {response}");
        assert!(response.ends_with("\r\n\r\nslow"));
        Ok::<(), anyhow::Error>(())
    }
    .await;

    stop_server(server);
    result.expect("slow upload client flow");
    upstream
        .await
        .expect("upstream task")
        .expect("upstream result");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn http1_downstream_read_timeout_rejects_stalled_upload() {
    install_rustls_provider();
    let upstream_listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("upstream bind");
    let upstream_addr = upstream_listener.local_addr().expect("upstream addr");
    let gateway_port = free_tcp_port();
    let snapshot = simple_http_snapshot(
        gateway_port,
        "/upload-timeout",
        upstream_addr.port() as u32,
        "HTTP",
    );
    let runtime = RuntimeOptions {
        enable_ipv6: false,
        downstream_read_timeout: Some(Duration::from_millis(75)),
        ..RuntimeOptions::default()
    };
    let plan = build_listener_plan(&snapshot.read(), &runtime, None).expect("plan");
    let log_path = temp_log_path("http1-downstream-read-timeout");
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
        assert!(request.starts_with("POST /upload-timeout HTTP/1.1\r\n"));
        let _ = timeout(Duration::from_millis(400), read_http_body(&mut stream, &request)).await;
        Ok::<(), anyhow::Error>(())
    });

    wait_for_listener(gateway_port).await;

    let result = async {
        let mut client = TcpStream::connect(("127.0.0.1", gateway_port)).await?;
        client
            .write_all(
                b"POST /upload-timeout HTTP/1.1\r\nHost: example.com\r\nContent-Length: 11\r\n\r\nhello",
            )
            .await?;
        sleep(Duration::from_millis(200)).await;
        let response = timeout(Duration::from_secs(1), read_http_response(&mut client)).await??;
        assert!(
            response.starts_with("HTTP/1.1 408"),
            "expected 408 response, got: {response}"
        );
        Ok::<(), anyhow::Error>(())
    }
    .await;

    stop_server(server);
    result.expect("stalled upload client flow");
    upstream
        .await
        .expect("upstream task")
        .expect("upstream result");

    let stats = wait_for_traffic_snapshot(&traffic, |stats| stats.total_events == 1).await;
    assert_eq!(stats.total_events, 1);
    assert_eq!(stats.status_4xx, 1);
    assert_eq!(stats.status_5xx, 0);
    assert_eq!(stats.response_flags.get("IT").copied(), Some(1));
    assert_eq!(stats.response_flags.get("UT").copied(), None);

    let log_contents = wait_for_log_contents(&log_path).await;
    assert!(log_contents.contains("\"status\":408"));
    assert!(log_contents.contains("\"responseFlags\":\"IT\""));
    assert!(
        !log_contents.contains("\"responseFlags\":\"UT\""),
        "downstream read timeout should not be logged as upstream timeout: {log_contents}"
    );

    shutdown_access_log_writer(&log_path.display().to_string());
    let _ = fs::remove_file(log_path);
}
