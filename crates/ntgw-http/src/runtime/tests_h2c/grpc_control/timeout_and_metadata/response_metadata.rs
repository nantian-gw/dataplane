#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn grpc_response_metadata_and_status_are_forwarded() {
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
        let (socket, _) = upstream_listener.accept().await?;
        let mut connection = h2server::handshake(socket).await?;
        let accepted_stream = connection
            .accept()
            .await
            .context("grpc stream should exist")?;
        let (request, mut respond) = accepted_stream.context("accept grpc request")?;
        assert_eq!(request.uri().path(), "/helloworld.Greeter/Watch");

        let response = Response::builder()
            .status(StatusCode::OK)
            .header("content-type", "application/grpc")
            .header("x-grpc-meta", "present")
            .body(())?;
        let mut send = respond.send_response(response, false)?;
        let mut trailers = HeaderMap::new();
        trailers.insert("grpc-status", "7".parse()?);
        trailers.insert("grpc-message", "permission denied".parse()?);
        send.send_trailers(trailers)?;
        drive_h2_server_io(&mut connection).await;
        Ok::<(), anyhow::Error>(())
    });

    wait_for_listener(gateway_port).await;

    let result = async {
        let stream = TcpStream::connect(("127.0.0.1", gateway_port)).await?;
        let (send_request, connection) = h2client::handshake(stream).await?;
        let connection_task = tokio::spawn(connection);
        let (response, _) = send_request.ready().await?.send_request(
            Request::builder()
                .method("POST")
                .uri("http://grpc.example.com/helloworld.Greeter/Watch")
                .version(http::Version::HTTP_2)
                .header("content-type", "application/grpc")
                .header("te", "trailers")
                .body(())?,
            true,
        )?;

        let response = response.await?;
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response
                .headers()
                .get("x-grpc-meta")
                .and_then(|value| value.to_str().ok()),
            Some("present")
        );
        let trailers = response
            .into_body()
            .trailers()
            .await?
            .expect("grpc trailers");
        assert_eq!(
            trailers
                .get("grpc-status")
                .and_then(|value| value.to_str().ok()),
            Some("7")
        );
        assert_eq!(
            trailers
                .get("grpc-message")
                .and_then(|value| value.to_str().ok()),
            Some("permission denied")
        );

        connection_task.abort();
        let _ = connection_task.await;
        Ok::<(), anyhow::Error>(())
    }
    .await;

    stop_server(server);
    result.expect("grpc metadata client flow");
    upstream
        .await
        .expect("upstream task")
        .expect("upstream result");
}
