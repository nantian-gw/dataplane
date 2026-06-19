#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn http1_chunked_request_trailers_are_dropped_by_upstream_h1_runtime() {
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
        assert_eq!(
            header_value(&request, "transfer-encoding").as_deref(),
            Some("chunked")
        );
        assert_eq!(header_value(&request, "te").as_deref(), Some("trailers"));
        assert_eq!(
            header_value(&request, "trailer").as_deref(),
            Some("x-checksum")
        );
        let (body, trailers) = read_chunked_body_and_trailers(&mut stream).await?;
        assert_eq!(body, b"hello");
        assert_eq!(trailers, "\r\n");
        stream
            .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 8\r\n\r\ntrailers")
            .await?;
        Ok::<(), anyhow::Error>(())
    });

    wait_for_listener(gateway_port).await;

    let result = async {
        let mut client = TcpStream::connect(("127.0.0.1", gateway_port)).await?;
        client
            .write_all(
                b"POST /upload HTTP/1.1\r\nHost: example.com\r\nTransfer-Encoding: chunked\r\nTE: trailers\r\nTrailer: x-checksum\r\n\r\n5\r\nhello\r\n0\r\nx-checksum: abc123\r\n\r\n",
            )
            .await?;
        let response = read_http_response(&mut client).await?;
        assert!(response.starts_with("HTTP/1.1 200"));
        assert!(response.ends_with("\r\n\r\ntrailers"));
        Ok::<(), anyhow::Error>(())
    }
    .await;

    stop_server(server);
    result.expect("chunked trailers compatibility flow");
    upstream
        .await
        .expect("upstream task")
        .expect("upstream result");
}
