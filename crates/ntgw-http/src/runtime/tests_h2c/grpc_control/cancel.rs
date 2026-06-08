#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn grpc_client_cancel_is_forwarded_to_upstream() {
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
    let response_frame = grpc_data_frame(b"partial");
    let response_frame_upstream = response_frame.clone();
    let (reset_seen_tx, reset_seen_rx) = oneshot::channel();
    let upstream = tokio::spawn(async move {
        let (socket, _) = upstream_listener.accept().await?;
        let mut connection = h2server::handshake(socket).await?;
        let accepted_stream = connection
            .accept()
            .await
            .context("grpc stream should exist")?;
        let (request, mut respond) = accepted_stream.context("accept grpc request")?;
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
        drive_h2_server_io(&mut connection).await;

        let reset_reason = timeout(Duration::from_secs(2), async {
            let reset = std::future::poll_fn(|cx| send.poll_reset(cx));
            tokio::pin!(reset);
            loop {
                tokio::select! {
                    reason = &mut reset => {
                        break Ok::<h2::Reason, anyhow::Error>(reason?);
                    }
                    accepted = connection.accept() => match accepted {
                        Some(stream) => {
                            let _ = stream.context("accept grpc request while waiting for reset")?;
                        }
                        None => return Err(anyhow!("grpc connection closed before client reset")),
                    }
                }
            }
        })
        .await??;
        assert_eq!(reset_reason, h2::Reason::CANCEL);
        let _ = reset_seen_tx.send(());
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
        let mut body = response.into_body();
        let first = timeout(Duration::from_secs(2), body.data())
            .await?
            .transpose()?
            .expect("grpc response frame");
        assert_eq!(first, bytes::Bytes::from(response_frame));

        send_stream.send_reset(h2::Reason::CANCEL);
        timeout(Duration::from_secs(2), reset_seen_rx).await??;
        connection_task.abort();
        let _ = connection_task.await;
        Ok::<(), anyhow::Error>(())
    }
    .await;

    stop_server(server);
    result.expect("grpc cancel client flow");
    upstream
        .await
        .expect("upstream task")
        .expect("upstream result");
}
