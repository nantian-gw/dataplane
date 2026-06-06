#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn http1_keepalive_reuses_downstream_and_upstream_connections() {
    install_rustls_provider();
    let upstream_listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("upstream bind");
    let upstream_addr = upstream_listener.local_addr().expect("upstream addr");
    let gateway_port = free_tcp_port();
    let snapshot = dual_protocol_snapshot(
        gateway_port,
        upstream_addr.port() as u32,
        free_tcp_port() as u32,
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

        let first = read_http_headers(&mut stream).await?;
        assert!(first.starts_with("GET /http HTTP/1.1\r\n"));
        stream
            .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 3\r\n\r\none")
            .await?;

        let second = read_http_headers(&mut stream).await?;
        assert!(second.starts_with("GET /http HTTP/1.1\r\n"));
        stream
            .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 3\r\nConnection: close\r\n\r\ntwo")
            .await?;
        Ok::<(), anyhow::Error>(())
    });

    wait_for_listener(gateway_port).await;

    let result = async {
        let mut client = TcpStream::connect(("127.0.0.1", gateway_port)).await?;

        client
            .write_all(b"GET /http HTTP/1.1\r\nHost: example.com\r\n\r\n")
            .await?;
        let first = read_http_response(&mut client).await?;
        assert!(first.starts_with("HTTP/1.1 200"));
        assert!(first.ends_with("\r\n\r\none"));

        client
            .write_all(b"GET /http HTTP/1.1\r\nHost: example.com\r\n\r\n")
            .await?;
        let second = read_http_response(&mut client).await?;
        assert!(second.starts_with("HTTP/1.1 200"));
        assert!(second.ends_with("\r\n\r\ntwo"));
        Ok::<(), anyhow::Error>(())
    }
    .await;

    stop_server(server);
    result.expect("keepalive client flow");
    upstream
        .await
        .expect("upstream task")
        .expect("upstream result");
}
