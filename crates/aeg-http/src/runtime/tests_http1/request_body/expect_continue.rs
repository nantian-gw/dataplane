#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn expect_100_continue_waits_for_upstream_ack_before_forwarding_body() {
    install_rustls_provider();
    let upstream_listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("upstream bind");
    let upstream_addr = upstream_listener.local_addr().expect("upstream addr");
    let gateway_port = free_tcp_port();
    let snapshot = simple_http_snapshot(gateway_port, "/upload", upstream_addr.port() as u32, "HTTP");
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
        assert!(request.starts_with("POST /upload HTTP/1.1\r\n"));
        assert_eq!(
            header_value(&request, "expect").as_deref(),
            Some("100-continue")
        );
        assert_eq!(
            header_value(&request, "content-length").as_deref(),
            Some("5")
        );
        stream.write_all(b"HTTP/1.1 100 Continue\r\n\r\n").await?;
        let body = read_http_body(&mut stream, &request).await?;
        assert_eq!(body, b"hello");
        stream
            .write_all(b"HTTP/1.1 201 Created\r\nContent-Length: 7\r\n\r\ncreated")
            .await?;
        Ok::<(), anyhow::Error>(())
    });

    wait_for_listener(gateway_port).await;

    let result = async {
        let mut client = TcpStream::connect(("127.0.0.1", gateway_port)).await?;
        client
            .write_all(
                b"POST /upload HTTP/1.1\r\nHost: example.com\r\nContent-Length: 5\r\nExpect: 100-continue\r\n\r\n",
            )
            .await?;
        let interim = read_http_headers(&mut client).await?;
        assert!(interim.starts_with("HTTP/1.1 100 Continue\r\n"));
        client.write_all(b"hello").await?;
        let response = read_http_response(&mut client).await?;
        assert!(response.starts_with("HTTP/1.1 201"));
        assert!(response.ends_with("\r\n\r\ncreated"));
        Ok::<(), anyhow::Error>(())
    }
    .await;

    stop_server(server);
    result.expect("expect continue client flow");
    upstream
        .await
        .expect("upstream task")
        .expect("upstream result");
}
