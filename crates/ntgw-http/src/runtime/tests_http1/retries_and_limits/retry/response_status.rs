#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn retryable_response_status_reselects_next_backend() {
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
    let plan = build_listener_plan(&snapshot.load(), &runtime, None).expect("plan");
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

    let failing = tokio::spawn(async move {
        let (mut stream, _) = failing_listener.accept().await?;
        let request = read_http_headers(&mut stream).await?;
        assert!(request.starts_with("GET /retry-status HTTP/1.1\r\n"));
        stream
            .write_all(b"HTTP/1.1 503 Service Unavailable\r\nContent-Length: 4\r\n\r\nfail")
            .await?;
        Ok::<(), anyhow::Error>(())
    });
    let healthy = tokio::spawn(async move {
        let (mut stream, _) = healthy_listener.accept().await?;
        let request = read_http_headers(&mut stream).await?;
        assert!(request.starts_with("GET /retry-status HTTP/1.1\r\n"));
        stream
            .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nok")
            .await?;
        Ok::<(), anyhow::Error>(())
    });

    wait_for_listener(gateway_port).await;

    let result = async {
        let mut client = TcpStream::connect(("127.0.0.1", gateway_port)).await?;
        client
            .write_all(b"GET /retry-status HTTP/1.1\r\nHost: example.com\r\n\r\n")
            .await?;
        let response = read_http_response(&mut client).await?;
        assert!(response.starts_with("HTTP/1.1 200"));
        assert!(response.ends_with("\r\n\r\nok"));
        Ok::<(), anyhow::Error>(())
    }
    .await;

    stop_server(server);
    result.expect("retry status client flow");
    failing
        .await
        .expect("failing task")
        .expect("failing result");
    healthy
        .await
        .expect("healthy task")
        .expect("healthy result");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn response_side_access_log_variables_capture_proxied_retry_response() {
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
    let snapshot = multi_backend_http_snapshot(
        gateway_port,
        "/response-vars",
        &[
            ("failing", failing_addr.port() as u32, "HTTP", 1),
            ("healthy", healthy_addr.port() as u32, "HTTP", 1),
        ],
        Some(RetryPolicy {
            codes: vec![502],
            attempts: 2,
            backoff: None,
        }),
    );
    {
        let mut current = (**snapshot.load()).clone();
        current.http_routes[0].annotations = BTreeMap::from([
            (
                "gateway.nantian.dev/access-log-mode".to_string(),
                "text".to_string(),
            ),
            (
                "gateway.nantian.dev/access-log-format".to_string(),
                r#"$scheme $remote_port "$sent_http_content_type" "$upstream_http_content_type" $upstream_status"#.to_string(),
            ),
        ]);
        current.http_routes[0].rules[0].filters = vec![Filter {
            filter_type: "ResponseHeaderModifier".to_string(),
            header_modifier: Some(HeaderModifier {
                set: vec![HeaderOperation {
                    name: "content-type".to_string().into(),
                    value: "text/plain".to_string(),
                }],
                ..HeaderModifier::default()
            }),
            ..Filter::default()
        }];
        current.rebuild_runtime_indexes();
        snapshot.store(Arc::new(current));
    }
    let runtime = RuntimeOptions {
        enable_ipv6: false,
        ..RuntimeOptions::default()
    };
    let plan = build_listener_plan(&snapshot.load(), &runtime, None).expect("plan");
    let log_path = temp_log_path("response-side-access-log-vars");
    let traffic = SharedTrafficStats::shared();
    let server = start_server(
        plan,
        snapshot.clone(),
        runtime,
        AccessLogOptions {
            enabled: true,
            path: log_path.display().to_string(),
            mode: ntgw_observability::AccessLogMode::Json,
            ..AccessLogOptions::default()
        },
        SessionPersistenceOptions::build(None, None).expect("session options"),
        traffic,
    )
    .expect("start server");

    let failing = tokio::spawn(async move {
        let (mut stream, _) = failing_listener.accept().await?;
        let request = read_http_headers(&mut stream).await?;
        assert!(request.starts_with("GET /response-vars HTTP/1.1\r\n"));
        stream
            .write_all(
                b"HTTP/1.1 502 Bad Gateway\r\nServer: orders-upstream\r\nContent-Length: 4\r\n\r\nfail",
            )
            .await?;
        Ok::<(), anyhow::Error>(())
    });
    let healthy = tokio::spawn(async move {
        let (mut stream, _) = healthy_listener.accept().await?;
        let request = read_http_headers(&mut stream).await?;
        assert!(request.starts_with("GET /response-vars HTTP/1.1\r\n"));
        stream
            .write_all(
                b"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nServer: orders-upstream\r\nContent-Length: 2\r\n\r\nok",
            )
            .await?;
        Ok::<(), anyhow::Error>(())
    });

    wait_for_listener(gateway_port).await;

    let result = async {
        let mut client = TcpStream::connect(("127.0.0.1", gateway_port)).await?;
        client
            .write_all(b"GET /response-vars HTTP/1.1\r\nHost: example.com\r\n\r\n")
            .await?;
        let response = read_http_response(&mut client).await?;
        let response_lower = response.to_ascii_lowercase();
        assert!(response.starts_with("HTTP/1.1 200"));
        assert!(response_lower.contains("content-type: text/plain\r\n"));
        assert!(!response_lower.contains("content-type: application/json\r\n"));
        assert!(response.ends_with("\r\n\r\nok"));
        Ok::<(), anyhow::Error>(())
    }
    .await;

    stop_server(server);
    result.expect("response vars client flow");
    failing
        .await
        .expect("failing task")
        .expect("failing result");
    healthy
        .await
        .expect("healthy task")
        .expect("healthy result");

    let log_contents = wait_for_log_contents(&log_path).await;
    let line = log_contents
        .lines()
        .find(|line| line.contains(r#""text/plain" "application/json" 502, 200"#))
        .expect("response-side access-log line");
    let mut parts = line.splitn(3, ' ');
    assert_eq!(parts.next(), Some("http"));
    let remote_port = parts.next().expect("remote port");
    assert!(
        remote_port.parse::<u16>().is_ok(),
        "expected numeric remote port, got {remote_port}"
    );
    assert!(line.contains(r#""text/plain" "application/json" 502, 200"#));

    shutdown_access_log_writer(&log_path.display().to_string());
    let _ = fs::remove_file(log_path);
}
