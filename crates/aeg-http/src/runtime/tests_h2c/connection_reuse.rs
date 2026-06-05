    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn http2_reuses_one_upstream_connection_across_multiple_requests() {
        install_rustls_provider();
        let upstream_listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("upstream bind");
        let upstream_addr = upstream_listener.local_addr().expect("upstream addr");
        let gateway_port = free_tcp_port();
        let snapshot =
            simple_http_snapshot(gateway_port, "/h2", upstream_addr.port() as u32, "H2C");
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
            let (socket, _) = upstream_listener.accept().await?;
            let mut connection = h2server::handshake(socket).await?;
            let (accepted_tx, mut accepted_rx) = mpsc::unbounded_channel();
            let (stop_tx, mut stop_rx) = oneshot::channel();
            let driver = tokio::spawn(async move {
                loop {
                    tokio::select! {
                        _ = &mut stop_rx => break Ok::<(), anyhow::Error>(()),
                        accepted = connection.accept() => match accepted {
                            Some(stream) => {
                                accepted_tx
                                    .send(stream.context("accept h2 request")?)
                                    .map_err(|_| anyhow!("h2 accept channel closed"))?;
                            }
                            None => break Ok(()),
                        }
                    }
                }
            });

            let mut seen = Vec::new();
            for _ in 0..2 {
                let (request, mut respond) = timeout(Duration::from_secs(2), accepted_rx.recv())
                    .await?
                    .ok_or_else(|| anyhow!("h2 stream should exist"))?;
                assert_eq!(request.uri().path(), "/h2");
                let stream_id = request
                    .headers()
                    .get("x-stream-id")
                    .and_then(|value| value.to_str().ok())
                    .map(str::to_string)
                    .expect("x-stream-id header");
                let body = match stream_id.as_str() {
                    "first" => bytes::Bytes::from_static(b"one"),
                    "second" => bytes::Bytes::from_static(b"two"),
                    other => return Err(anyhow!("unexpected stream id: {other}")),
                };
                let response = Response::builder().status(StatusCode::OK).body(())?;
                let mut stream = respond.send_response(response, false)?;
                stream.send_data(body, true)?;
                seen.push(stream_id);
            }

            assert!(seen.contains(&"first".to_string()));
            assert!(seen.contains(&"second".to_string()));
            sleep(Duration::from_millis(50)).await;
            let _ = stop_tx.send(());
            let _ = driver.await;

            Ok::<(), anyhow::Error>(())
        });

        wait_for_listener(gateway_port).await;

        let result = async {
            let stream = TcpStream::connect(("127.0.0.1", gateway_port)).await?;
            let (send_request, connection) = h2client::handshake(stream).await?;
            let connection_task = tokio::spawn(connection);
            let mut send_request = send_request.ready().await?;
            let (first_response, _) = send_request.send_request(
                Request::builder()
                    .method("GET")
                    .uri("http://example.com/h2")
                    .version(http::Version::HTTP_2)
                    .header("x-stream-id", "first")
                    .body(())?,
                true,
            )?;
            let first = timeout(Duration::from_secs(2), first_response).await??;
            let first_body =
                timeout(Duration::from_secs(2), read_h2_body(first.into_body())).await??;
            assert_eq!(first_body, b"one");

            let mut send_request = send_request.ready().await?;
            let (second_response, _) = send_request.send_request(
                Request::builder()
                    .method("GET")
                    .uri("http://example.com/h2")
                    .version(http::Version::HTTP_2)
                    .header("x-stream-id", "second")
                    .body(())?,
                true,
            )?;

            let second = timeout(Duration::from_secs(2), second_response).await??;
            let second_body =
                timeout(Duration::from_secs(2), read_h2_body(second.into_body())).await??;
            assert_eq!(second_body, b"two");

            drop(send_request);
            connection_task.abort();
            let _ = connection_task.await;
            Ok::<(), anyhow::Error>(())
        }
        .await;

        stop_server(server);
        result.expect("http2 client flow");
        upstream
            .await
            .expect("upstream task")
            .expect("upstream result");
    }
