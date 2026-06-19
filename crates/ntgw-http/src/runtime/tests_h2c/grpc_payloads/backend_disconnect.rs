#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn grpc_backend_disconnect_mid_stream_surfaces_as_downstream_error() {
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

    let first_frame = grpc_data_frame(b"partial");
    let first_frame_upstream = first_frame.clone();
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
            .body(())?;
        let mut send = respond.send_response(response, false)?;
        send.send_data(bytes::Bytes::from(first_frame_upstream), false)?;
        drive_h2_server_io(&mut connection).await;
        drop(send);
        drop(connection);
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
        let mut body = response.into_body();
        let first = body.data().await.expect("first grpc frame");
        assert_eq!(first?, bytes::Bytes::from(first_frame));

        match body.data().await {
            Some(Err(_)) => {}
            Some(Ok(chunk)) => {
                return Err(anyhow!(
                    "expected backend disconnect, got extra chunk: {chunk:?}"
                ));
            }
            None => {
                let trailers = body.trailers().await;
                if trailers.is_ok() {
                    return Err(anyhow!(
                        "expected grpc backend disconnect to surface as stream error"
                    ));
                }
            }
        }

        connection_task.abort();
        let _ = connection_task.await;
        Ok::<(), anyhow::Error>(())
    }
    .await;

    stop_server(server);
    result.expect("grpc disconnect client flow");
    upstream
        .await
        .expect("upstream task")
        .expect("upstream result");
}
