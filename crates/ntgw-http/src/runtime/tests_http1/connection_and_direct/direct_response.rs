#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn direct_response_short_circuits_before_upstream_and_emits_observability() {
    install_rustls_provider();
    let gateway_port = free_tcp_port();
    let snapshot = direct_response_snapshot(gateway_port, "/direct");
    let runtime = RuntimeOptions {
        enable_ipv6: false,
        ..RuntimeOptions::default()
    };
    let plan = build_listener_plan(&snapshot.load(), &runtime, None).expect("plan");
    let log_path = temp_log_path("direct-response");
    let traffic = SharedTrafficStats::shared();
    let server = start_server(
        plan,
        snapshot.clone(),
        runtime,
        AccessLogOptions {
            enabled: true,
            path: log_path.display().to_string(),
            mode: ntgw_observability::AccessLogMode::Text,
            format: "$sent_http_content_type $scheme $remote_port".to_string(),
            ..AccessLogOptions::default()
        },
        SessionPersistenceOptions::build(None, None).expect("session options"),
        traffic.clone(),
    )
    .expect("start server");

    wait_for_listener(gateway_port).await;

    let result = async {
        let mut client = TcpStream::connect(("127.0.0.1", gateway_port)).await?;
        client
            .write_all(
                b"GET /direct HTTP/1.1\r\nHost: example.com\r\nX-Request-Id: req-123\r\n\r\n",
            )
            .await?;
        let response = read_http_response(&mut client).await?;
        assert!(response.starts_with("HTTP/1.1 202"));
        assert!(response
            .to_ascii_lowercase()
            .contains("content-type: text/plain\r\n"));
        assert!(response
            .to_ascii_lowercase()
            .contains("x-lifecycle-stage: request-filter\r\n"));
        assert!(response.ends_with("\r\n\r\ndirect response"));
        Ok::<(), anyhow::Error>(())
    }
    .await;

    stop_server(server);
    result.expect("direct response client flow");

    let stats = wait_for_traffic_snapshot(&traffic, |stats| {
        stats.total_events == 1 && stats.status_2xx == 1
    })
    .await;
    assert_eq!(stats.total_events, 1);
    assert_eq!(stats.status_2xx, 1);
    assert_eq!(stats.total_retry_attempts, 0);

    let log_contents = wait_for_log_contents(&log_path).await;
    let line = log_contents
        .lines()
        .find(|line| line.starts_with("text/plain http "))
        .expect("direct response access-log line");
    let remote_port = line
        .strip_prefix("text/plain http ")
        .expect("remote port prefix")
        .trim();
    assert!(
        remote_port.parse::<u16>().is_ok(),
        "expected numeric remote port, got {remote_port}"
    );

    shutdown_access_log_writer(&log_path.display().to_string());
    let _ = fs::remove_file(log_path);
}
