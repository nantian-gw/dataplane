#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn http_client_cancel_before_response_headers_records_client_closed() {
    install_rustls_provider();
    let upstream_listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("upstream bind");
    let upstream_addr = upstream_listener.local_addr().expect("upstream addr");
    let gateway_port = free_tcp_port();
    let snapshot = simple_http_snapshot(
        gateway_port,
        "/console/api/rule-generate",
        upstream_addr.port() as u32,
        "HTTP",
    );
    let runtime = RuntimeOptions {
        enable_ipv6: false,
        ..RuntimeOptions::default()
    };
    let plan = build_listener_plan(&snapshot.load(), &runtime, None).expect("plan");
    let log_path = temp_log_path("http-client-cancel-before-response-headers");
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

    let request_body = b"{\"prompt\":\"slow rule\"}";
    let expected_request_body = request_body.to_vec();
    let (request_received_tx, request_received_rx) = oneshot::channel();
    let upstream = tokio::spawn(async move {
        let (mut stream, _) = upstream_listener.accept().await?;
        let request = read_http_headers(&mut stream).await?;
        assert!(request.starts_with("POST /console/api/rule-generate HTTP/1.1\r\n"));
        let mut body = vec![0; expected_request_body.len()];
        stream.read_exact(&mut body).await?;
        assert_eq!(body, expected_request_body);
        let _ = request_received_tx.send(());
        sleep(Duration::from_millis(100)).await;
        let _ = stream
            .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok")
            .await;
        Ok::<(), anyhow::Error>(())
    });

    wait_for_listener(gateway_port).await;

    let result = async {
        let mut client = TcpStream::connect(("127.0.0.1", gateway_port)).await?;
        client
            .write_all(
                format!(
                    "POST /console/api/rule-generate HTTP/1.1\r\nHost: dify.kubernetes.com\r\nContent-Length: {}\r\n\r\n",
                    request_body.len()
                )
                .as_bytes(),
            )
            .await?;
        client.write_all(request_body).await?;
        client.flush().await?;
        request_received_rx.await?;
        drop(client);
        Ok::<(), anyhow::Error>(())
    }
    .await;

    result.expect("pre-header client cancel flow");
    upstream
        .await
        .expect("upstream task")
        .expect("upstream result");

    let stats = wait_for_traffic_snapshot(&traffic, |stats| stats.total_events == 1).await;
    stop_server(server);
    assert_eq!(stats.total_events, 1);
    assert_eq!(stats.status_4xx, 1);
    assert_eq!(stats.status_5xx, 0);
    assert_eq!(stats.total_retry_attempts, 0);

    let log_contents = wait_for_log_contents(&log_path).await;
    assert!(log_contents.contains("\"status\":499"));
    assert!(log_contents.contains("\"retryAttempts\":0"));
    assert!(
        log_contents.contains("\"responseFlags\":\"DC\""),
        "pre-header client cancel should be logged as downstream close, got: {log_contents}"
    );

    shutdown_access_log_writer(&log_path.display().to_string());
    let _ = fs::remove_file(log_path);
}
