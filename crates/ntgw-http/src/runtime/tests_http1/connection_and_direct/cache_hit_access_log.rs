#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn cache_hit_preserves_sent_response_access_log_headers() {
    install_rustls_provider();
    let upstream_listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("upstream bind");
    let upstream_addr = upstream_listener.local_addr().expect("upstream addr");
    let gateway_port = free_tcp_port();
    let snapshot = simple_http_snapshot(gateway_port, "/cached", upstream_addr.port() as u32, "HTTP");
    let runtime = RuntimeOptions {
        enable_ipv6: false,
        cache: crate::cache::CacheManager::new(crate::cache::CacheOptions {
            enabled: true,
            max_size_bytes: 16 * 1024 * 1024,
            max_entry_size_bytes: 1024 * 1024,
            default_ttl: Duration::from_secs(60),
        }),
        ..RuntimeOptions::default()
    };
    let plan = build_listener_plan(&snapshot.load(), &runtime, None).expect("plan");
    let log_path = temp_log_path("cache-hit-access-log");
    let server = start_server(
        plan,
        snapshot.clone(),
        runtime,
        AccessLogOptions {
            enabled: true,
            mode: ntgw_observability::AccessLogMode::Text,
            path: log_path.display().to_string(),
            format: "$sent_http_content_type $upstream_status".to_string(),
            ..AccessLogOptions::default()
        },
        SessionPersistenceOptions::build(None, None).expect("session options"),
        SharedTrafficStats::shared(),
    )
    .expect("start server");

    let upstream = tokio::spawn(async move {
        let (mut stream, _) = upstream_listener.accept().await?;
        let request = read_http_headers(&mut stream).await?;
        assert!(request.starts_with("GET /cached HTTP/1.1\r\n"));
        stream
            .write_all(
                b"HTTP/1.1 200 OK\r\nDate: Thu, 26 Apr 2018 05:42:05 GMT\r\nContent-Type: text/plain\r\nCache-Control: public, max-age=60\r\nContent-Length: 5\r\nConnection: keep-alive\r\n\r\nhello",
            )
            .await?;
        timeout(Duration::from_millis(250), upstream_listener.accept())
            .await
            .expect_err("second request should be served from cache");
        Ok::<(), anyhow::Error>(())
    });

    wait_for_listener(gateway_port).await;

    let result = async {
        let mut first = TcpStream::connect(("127.0.0.1", gateway_port)).await?;
        first
            .write_all(b"GET /cached HTTP/1.1\r\nHost: example.com\r\n\r\n")
            .await?;
        let first_response = read_http_response(&mut first).await?;
        assert!(first_response.starts_with("HTTP/1.1 200"));
        assert!(first_response.to_ascii_lowercase().contains("content-type: text/plain\r\n"));
        assert!(first_response.ends_with("\r\n\r\nhello"));
        drop(first);

        let first_log = wait_for_log_contents(&log_path).await;
        assert!(first_log.contains("text/plain 200"));

        let mut second = TcpStream::connect(("127.0.0.1", gateway_port)).await?;
        second
            .write_all(b"GET /cached HTTP/1.1\r\nHost: example.com\r\n\r\n")
            .await?;
        let second_response = read_http_response(&mut second).await?;
        assert!(second_response.starts_with("HTTP/1.1 200"));
        assert!(second_response.to_ascii_lowercase().contains("content-type: text/plain\r\n"));
        assert!(second_response.ends_with("\r\n\r\nhello"));
        Ok::<(), anyhow::Error>(())
    }
    .await;

    stop_server(server);
    result.expect("cache client flow");
    upstream.await.expect("upstream task").expect("upstream result");

    let log_contents = wait_for_log_contents(&log_path).await;
    assert!(log_contents.contains("text/plain 200"));
    assert!(log_contents.contains("text/plain -"));

    shutdown_access_log_writer(&log_path.display().to_string());
    let _ = fs::remove_file(log_path);
}
