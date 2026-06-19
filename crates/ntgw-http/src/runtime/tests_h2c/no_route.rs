#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn http2_unmatched_route_returns_404_without_upstream() {
    install_rustls_provider();
    let upstream_listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("upstream bind");
    let upstream_addr = upstream_listener.local_addr().expect("upstream addr");
    let gateway_port = free_tcp_port();
    let snapshot =
        simple_http_snapshot(gateway_port, "/matched", upstream_addr.port() as u32, "H2C");
    let runtime = RuntimeOptions {
        enable_ipv6: false,
        ..RuntimeOptions::default()
    };
    let plan = build_listener_plan(&snapshot.load(), &runtime, None).expect("plan");
    let log_path = temp_log_path("http2-no-route-access-log");
    let server = start_server(
        plan,
        snapshot.clone(),
        runtime,
        AccessLogOptions {
            enabled: true,
            path: log_path.display().to_string(),
            mode: ntgw_observability::AccessLogMode::Text,
            format: "$sent_http_content_length $upstream_status".to_string(),
            ..AccessLogOptions::default()
        },
        SessionPersistenceOptions::build(None, None).expect("session options"),
        SharedTrafficStats::shared(),
    )
    .expect("start server");

    wait_for_listener(gateway_port).await;

    let result = async {
        let stream = TcpStream::connect(("127.0.0.1", gateway_port)).await?;
        let (send_request, connection) = h2client::handshake(stream).await?;
        let connection_task = tokio::spawn(connection);
        let (response, _) = send_request.ready().await?.send_request(
            Request::builder()
                .method("GET")
                .uri("http://example.com/missing")
                .version(http::Version::HTTP_2)
                .body(())?,
            true,
        )?;

        let response = timeout(Duration::from_secs(2), response).await??;
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        let body = timeout(Duration::from_secs(2), read_h2_body(response.into_body())).await??;
        assert!(!body.is_empty(), "expected an explicit 404 body");

        connection_task.abort();
        let _ = connection_task.await;
        Ok::<(), anyhow::Error>(())
    }
    .await;

    stop_server(server);
    result.expect("http2 no-route client flow");
    drop(upstream_listener);

    let log_contents = wait_for_log_contents(&log_path).await;
    assert!(log_contents.contains("15 -"));

    shutdown_access_log_writer(&log_path.display().to_string());
    let _ = fs::remove_file(log_path);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn grpc_unmatched_route_returns_unimplemented_without_upstream() {
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

    wait_for_listener(gateway_port).await;

    let result = async {
        let stream = TcpStream::connect(("127.0.0.1", gateway_port)).await?;
        let (send_request, connection) = h2client::handshake(stream).await?;
        let connection_task = tokio::spawn(connection);
        let (response, _) = send_request.ready().await?.send_request(
            Request::builder()
                .method("POST")
                .uri("http://grpc.example.com/helloworld.Greeter/Missing")
                .version(http::Version::HTTP_2)
                .header("content-type", "application/grpc")
                .header("te", "trailers")
                .body(())?,
            true,
        )?;

        let response = timeout(Duration::from_secs(2), response).await??;
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response
                .headers()
                .get("content-type")
                .and_then(|value| value.to_str().ok()),
            Some("application/grpc")
        );
        let trailers = timeout(Duration::from_secs(2), response.into_body().trailers())
            .await??
            .expect("grpc trailers");
        assert_eq!(
            trailers
                .get("grpc-status")
                .and_then(|value| value.to_str().ok()),
            Some("12")
        );

        connection_task.abort();
        let _ = connection_task.await;
        Ok::<(), anyhow::Error>(())
    }
    .await;

    stop_server(server);
    result.expect("grpc no-route client flow");
    drop(upstream_listener);
}
