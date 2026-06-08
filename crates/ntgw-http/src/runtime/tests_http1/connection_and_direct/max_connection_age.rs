#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn http1_max_connection_age_closes_downstream_after_current_request() {
    install_rustls_provider();
    let upstream_listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("upstream bind");
    let upstream_addr = upstream_listener.local_addr().expect("upstream addr");
    let gateway_port = free_tcp_port();
    let snapshot = simple_http_snapshot(gateway_port, "/age", upstream_addr.port() as u32, "HTTP");
    let runtime = RuntimeOptions {
        enable_ipv6: false,
        downstream_max_connection_age: Some(Duration::from_millis(75)),
        ..RuntimeOptions::default()
    };
    let plan = build_listener_plan(&snapshot.read(), &runtime, None).expect("plan");
    let traffic = SharedTrafficStats::shared();
    let server = start_server(
        plan,
        snapshot.clone(),
        runtime,
        AccessLogOptions {
            enabled: false,
            ..AccessLogOptions::default()
        },
        SessionPersistenceOptions::build(None, None).expect("session options"),
        traffic.clone(),
    )
    .expect("start server");

    let upstream = tokio::spawn(async move {
        let (mut stream, _) = upstream_listener.accept().await?;

        let first = read_http_headers(&mut stream).await?;
        assert!(first.starts_with("GET /age HTTP/1.1\r\n"));
        stream
            .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 3\r\n\r\none")
            .await?;

        let second = read_http_headers(&mut stream).await?;
        assert!(second.starts_with("GET /age HTTP/1.1\r\n"));
        stream
            .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 3\r\n\r\ntwo")
            .await?;
        Ok::<(), anyhow::Error>(())
    });

    wait_for_listener(gateway_port).await;

    let result = async {
        let mut client = TcpStream::connect(("127.0.0.1", gateway_port)).await?;

        client
            .write_all(b"GET /age HTTP/1.1\r\nHost: example.com\r\n\r\n")
            .await?;
        let first = read_http_response(&mut client).await?;
        assert!(first.starts_with("HTTP/1.1 200"));
        assert!(first.ends_with("\r\n\r\none"));

        sleep(Duration::from_millis(120)).await;

        client
            .write_all(b"GET /age HTTP/1.1\r\nHost: example.com\r\n\r\n")
            .await?;
        let second = read_http_response(&mut client).await?;
        assert!(second.starts_with("HTTP/1.1 200"));
        assert!(
            second
                .lines()
                .any(|line| line.eq_ignore_ascii_case("connection: close")),
            "max-age response should close the downstream connection: {second:?}"
        );
        assert!(second.ends_with("\r\n\r\ntwo"));

        let mut eof = [0; 1];
        let read = timeout(Duration::from_secs(2), client.read(&mut eof)).await??;
        assert_eq!(read, 0, "client should observe max-age downstream close");
        Ok::<(), anyhow::Error>(())
    }
    .await;

    let stats = wait_for_traffic_snapshot(&traffic, |snapshot| {
        snapshot.response_flags.get("MA").copied() == Some(1)
    })
    .await;

    stop_server(server);
    result.expect("client flow");
    upstream
        .await
        .expect("upstream task")
        .expect("upstream result");
    assert_eq!(stats.response_flags.get("MA").copied(), Some(1));
}
