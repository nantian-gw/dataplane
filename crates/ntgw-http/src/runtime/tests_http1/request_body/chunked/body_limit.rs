#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn http1_request_body_limit_rejects_chunked_upload_once_limit_is_exceeded() {
    install_rustls_provider();
    let upstream_listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("upstream bind");
    let upstream_addr = upstream_listener.local_addr().expect("upstream addr");
    let gateway_port = free_tcp_port();
    let snapshot =
        simple_http_snapshot(gateway_port, "/upload", upstream_addr.port() as u32, "HTTP");
    let runtime = RuntimeOptions {
        enable_ipv6: false,
        max_request_body_bytes: 5,
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
        let request = read_http_headers(&mut stream).await?;
        assert!(request.starts_with("POST /upload HTTP/1.1\r\n"));
        let body = timeout(
            Duration::from_millis(500),
            read_http_body(&mut stream, &request),
        )
        .await;
        if let Ok(Ok(body)) = body {
            assert_eq!(body, b"hello world");
            stream
                .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nok")
                .await?;
        }
        Ok::<(), anyhow::Error>(())
    });

    wait_for_listener(gateway_port).await;

    let result = async {
        let mut client = TcpStream::connect(("127.0.0.1", gateway_port)).await?;
        client
            .write_all(
                b"POST /upload HTTP/1.1\r\nHost: example.com\r\nTransfer-Encoding: chunked\r\n\r\n5\r\nhello\r\n6\r\n world\r\n0\r\n\r\n",
            )
            .await?;
        let response = read_http_response(&mut client).await?;
        assert!(
            response.starts_with("HTTP/1.1 413"),
            "expected 413 response, got: {response}"
        );
        Ok::<(), anyhow::Error>(())
    }
    .await;

    stop_server(server);
    result.expect("oversized chunked upload client flow");
    upstream
        .await
        .expect("upstream task")
        .expect("upstream result");
}
