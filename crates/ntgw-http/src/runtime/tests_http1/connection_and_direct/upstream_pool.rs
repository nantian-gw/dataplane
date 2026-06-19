#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn upstream_connection_pool_reuses_idle_http1_connection_across_clients() {
    install_rustls_provider();
    let upstream_listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("upstream bind");
    let upstream_addr = upstream_listener.local_addr().expect("upstream addr");
    let gateway_port = free_tcp_port();
    let snapshot =
        simple_http_snapshot(gateway_port, "/reuse", upstream_addr.port() as u32, "HTTP");
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

    let upstream = tokio::spawn(async move {
        let (mut stream, _) = upstream_listener.accept().await?;

        let first = read_http_headers(&mut stream).await?;
        assert!(first.starts_with("GET /reuse HTTP/1.1\r\n"));
        stream
            .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 3\r\n\r\none")
            .await?;

        let second = read_http_headers(&mut stream).await?;
        assert!(second.starts_with("GET /reuse HTTP/1.1\r\n"));
        stream
            .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 3\r\nConnection: close\r\n\r\ntwo")
            .await?;
        Ok::<(), anyhow::Error>(())
    });

    wait_for_listener(gateway_port).await;

    let result = async {
        let mut first_client = TcpStream::connect(("127.0.0.1", gateway_port)).await?;
        first_client
            .write_all(b"GET /reuse HTTP/1.1\r\nHost: example.com\r\n\r\n")
            .await?;
        let first = read_http_response(&mut first_client).await?;
        assert!(first.starts_with("HTTP/1.1 200"));
        assert!(first.ends_with("\r\n\r\none"));
        drop(first_client);

        let mut second_client = TcpStream::connect(("127.0.0.1", gateway_port)).await?;
        second_client
            .write_all(b"GET /reuse HTTP/1.1\r\nHost: example.com\r\n\r\n")
            .await?;
        let second = read_http_response(&mut second_client).await?;
        assert!(second.starts_with("HTTP/1.1 200"));
        assert!(second.ends_with("\r\n\r\ntwo"));
        Ok::<(), anyhow::Error>(())
    }
    .await;

    stop_server(server);
    result.expect("client flow");
    upstream
        .await
        .expect("upstream task")
        .expect("upstream result");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn downstream_connection_close_does_not_disable_upstream_pool_reuse() {
    install_rustls_provider();
    let upstream_listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("upstream bind");
    let upstream_addr = upstream_listener.local_addr().expect("upstream addr");
    let gateway_port = free_tcp_port();
    let snapshot =
        simple_http_snapshot(gateway_port, "/reuse", upstream_addr.port() as u32, "HTTP");
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

    let upstream = tokio::spawn(async move {
        let (mut stream, _) = upstream_listener.accept().await?;

        let first = read_http_headers(&mut stream).await?;
        assert!(first.starts_with("GET /reuse HTTP/1.1\r\n"));
        assert!(
            !first
                .lines()
                .any(|line| line.eq_ignore_ascii_case("connection: close")),
            "downstream Connection: close must not be forwarded upstream: {first:?}"
        );
        stream
            .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 3\r\n\r\none")
            .await?;

        let second = timeout(Duration::from_secs(2), read_http_headers(&mut stream))
            .await
            .context("second request should reuse the first upstream connection")??;
        assert!(second.starts_with("GET /reuse HTTP/1.1\r\n"));
        stream
            .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 3\r\nConnection: close\r\n\r\ntwo")
            .await?;
        Ok::<(), anyhow::Error>(())
    });

    wait_for_listener(gateway_port).await;

    let result = async {
        let mut first_client = TcpStream::connect(("127.0.0.1", gateway_port)).await?;
        first_client
            .write_all(b"GET /reuse HTTP/1.1\r\nHost: example.com\r\nConnection: close\r\n\r\n")
            .await?;
        let first = read_http_response(&mut first_client).await?;
        assert!(first.starts_with("HTTP/1.1 200"));
        assert!(first.ends_with("\r\n\r\none"));

        let mut second_client = TcpStream::connect(("127.0.0.1", gateway_port)).await?;
        second_client
            .write_all(b"GET /reuse HTTP/1.1\r\nHost: example.com\r\n\r\n")
            .await?;
        let second = read_http_response(&mut second_client).await?;
        assert!(second.starts_with("HTTP/1.1 200"));
        assert!(second.ends_with("\r\n\r\ntwo"));
        Ok::<(), anyhow::Error>(())
    }
    .await;

    stop_server(server);
    result.expect("client flow");
    upstream
        .await
        .expect("upstream task")
        .expect("upstream result");
}
