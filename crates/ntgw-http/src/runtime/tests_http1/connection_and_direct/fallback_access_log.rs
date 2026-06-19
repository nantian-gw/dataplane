#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn fallback_selection_access_log_captures_connection_fields_for_proxied_request() {
    install_rustls_provider();
    let upstream_listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("upstream bind");
    let upstream_addr = upstream_listener.local_addr().expect("upstream addr");
    let gateway_port = free_tcp_port();
    let snapshot = simple_http_snapshot(gateway_port, "/unreachable", upstream_addr.port() as u32, "HTTP");
    {
        let mut current = (**snapshot.load()).clone();
        current.http_routes.clear();
        current.listeners[0].attached_routes.clear();
        current.rebuild_runtime_indexes();
        snapshot.store(Arc::new(current));
    }
    let runtime = RuntimeOptions {
        enable_ipv6: false,
        ..RuntimeOptions::default()
    };
    let plan = build_listener_plan(&snapshot.load(), &runtime, None).expect("plan");
    let log_path = temp_log_path("fallback-access-log-connection-fields");
    let server = start_server(
        plan,
        snapshot.clone(),
        runtime,
        AccessLogOptions {
            enabled: true,
            path: log_path.display().to_string(),
            mode: ntgw_observability::AccessLogMode::Text,
            format: "$scheme $remote_port".to_string(),
            ..AccessLogOptions::default()
        },
        SessionPersistenceOptions::build(None, None).expect("session options"),
        SharedTrafficStats::shared(),
    )
    .expect("start server");

    let upstream = tokio::spawn(async move {
        let (mut stream, _) = upstream_listener.accept().await?;
        let request = read_http_headers(&mut stream).await?;
        assert!(request.starts_with("GET /fallback HTTP/1.1\r\n"));
        stream
            .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nok")
            .await?;
        Ok::<(), anyhow::Error>(())
    });

    wait_for_listener(gateway_port).await;

    let result = async {
        let mut client = TcpStream::connect(("127.0.0.1", gateway_port)).await?;
        client
            .write_all(b"GET /fallback HTTP/1.1\r\nHost: example.com\r\n\r\n")
            .await?;
        let response = read_http_response(&mut client).await?;
        assert!(response.starts_with("HTTP/1.1 200"));
        assert!(response.ends_with("\r\n\r\nok"));
        Ok::<(), anyhow::Error>(())
    }
    .await;

    stop_server(server);
    result.expect("fallback selection client flow");
    upstream
        .await
        .expect("upstream task")
        .expect("upstream result");

    let log_contents = wait_for_log_contents(&log_path).await;
    let line = log_contents
        .lines()
        .find(|line| line.starts_with("http "))
        .expect("fallback access-log line");
    let remote_port = line
        .strip_prefix("http ")
        .expect("http prefix")
        .trim();
    assert!(
        remote_port.parse::<u16>().is_ok(),
        "expected numeric remote port, got {remote_port}"
    );

    shutdown_access_log_writer(&log_path.display().to_string());
    let _ = fs::remove_file(log_path);
}
