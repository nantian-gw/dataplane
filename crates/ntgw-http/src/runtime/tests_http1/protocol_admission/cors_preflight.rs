#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn cors_preflight_is_handled_without_proxying_to_upstream() {
    install_rustls_provider();
    let upstream_listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("upstream bind");
    let upstream_addr = upstream_listener.local_addr().expect("upstream addr");
    let gateway_port = free_tcp_port();
    let snapshot = cors_http_snapshot(gateway_port, "/cors", upstream_addr.port() as u32);
    let runtime = RuntimeOptions {
        enable_ipv6: false,
        ..RuntimeOptions::default()
    };
    let plan = build_listener_plan(&snapshot.load(), &runtime, None).expect("plan");
    let log_path = temp_log_path("cors-preflight-access-log");
    let server = start_server(
        plan,
        snapshot.clone(),
        runtime,
        AccessLogOptions {
            enabled: true,
            path: log_path.display().to_string(),
            mode: ntgw_observability::AccessLogMode::Text,
            format: "$sent_http_access_control_allow_origin $upstream_status".to_string(),
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
                b"OPTIONS /cors HTTP/1.1\r\nHost: example.com\r\nOrigin: https://app.example\r\nAccess-Control-Request-Method: POST\r\n\r\n",
            )
            .await?;
        let response = read_http_response(&mut client).await?;
        assert!(response.starts_with("HTTP/1.1 204"));
        let lower = response.to_ascii_lowercase();
        assert!(lower.contains("access-control-allow-origin: https://app.example\r\n"));
        assert!(lower.contains("access-control-allow-methods: get, post\r\n"));
        assert!(lower.contains("access-control-allow-headers: authorization, content-type\r\n"));
        assert!(lower.contains("access-control-max-age: 600\r\n"));
        assert!(response.ends_with("\r\n\r\n"));
        Ok::<(), anyhow::Error>(())
    }
    .await;

    stop_server(server);
    result.expect("cors preflight client flow");

    let no_upstream = timeout(Duration::from_millis(300), upstream_listener.accept()).await;
    assert!(no_upstream.is_err(), "preflight should not reach upstream");

    let log_contents = wait_for_log_contents(&log_path).await;
    assert!(log_contents.contains("https://app.example -"));

    shutdown_access_log_writer(&log_path.display().to_string());
    let _ = fs::remove_file(log_path);
}
