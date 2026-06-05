#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn grpc_unary_request_body_is_forwarded_over_h2c() {
    install_rustls_provider();
    let upstream_listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("upstream bind");
    let upstream_addr = upstream_listener.local_addr().expect("upstream addr");
    let gateway_port = free_tcp_port();
    let snapshot = grpc_h2c_snapshot(gateway_port, upstream_addr.port() as u32);
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

    let request_frame = grpc_data_frame(b"request");
    let request_frame_upstream = request_frame.clone();
    let response_frame = grpc_data_frame(b"response");
    let response_frame_upstream = response_frame.clone();
    let upstream = tokio::spawn(async move {
        let (socket, _) = upstream_listener.accept().await?;
        let mut connection = h2server::handshake(socket).await?;
        let accepted_stream = connection
            .accept()
            .await
            .context("grpc stream should exist")?;
        let (request, mut respond) = accepted_stream.context("accept grpc request")?;
        assert_eq!(request.method(), "POST");
        assert_eq!(request.uri().path(), "/helloworld.Greeter/Watch");

        let mut body = request.into_body();
        drive_h2_server_io(&mut connection).await;
        let collected = timeout(Duration::from_secs(2), read_h2_body_stream(&mut body)).await??;
        assert_eq!(collected, request_frame_upstream);

        let response = Response::builder()
            .status(StatusCode::OK)
            .header("content-type", "application/grpc")
            .body(())?;
        let mut send = respond.send_response(response, false)?;
        send.send_data(bytes::Bytes::from(response_frame_upstream), false)?;
        let mut trailers = HeaderMap::new();
        trailers.insert("grpc-status", "0".parse()?);
        send.send_trailers(trailers)?;
        drive_h2_server_io(&mut connection).await;
        Ok::<(), anyhow::Error>(())
    });

    wait_for_listener(gateway_port).await;

    let result = async {
        let stream = TcpStream::connect(("127.0.0.1", gateway_port)).await?;
        let (send_request, connection) = h2client::handshake(stream).await?;
        let connection_task = tokio::spawn(connection);
        let (response, mut send_stream) = send_request.ready().await?.send_request(
            Request::builder()
                .method("POST")
                .uri("http://grpc.example.com/helloworld.Greeter/Watch")
                .version(http::Version::HTTP_2)
                .header("content-type", "application/grpc")
                .header("te", "trailers")
                .body(())?,
            false,
        )?;
        send_stream.send_data(bytes::Bytes::from(request_frame), true)?;

        let response = timeout(Duration::from_secs(2), response).await??;
        assert_eq!(response.status(), StatusCode::OK);
        let mut body = response.into_body();
        let collected = timeout(Duration::from_secs(2), read_h2_body_stream(&mut body)).await??;
        assert_eq!(collected, response_frame);
        let trailers = timeout(Duration::from_secs(2), body.trailers())
            .await??
            .expect("grpc trailers");
        assert_eq!(
            trailers
                .get("grpc-status")
                .and_then(|value| value.to_str().ok()),
            Some("0")
        );

        connection_task.abort();
        let _ = connection_task.await;
        Ok::<(), anyhow::Error>(())
    }
    .await;

    stop_server(server);
    result.expect("grpc unary body client flow");
    upstream
        .await
        .expect("upstream task")
        .expect("upstream result");
}
