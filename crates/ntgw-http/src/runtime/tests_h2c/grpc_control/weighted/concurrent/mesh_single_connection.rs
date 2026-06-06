#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn mesh_grpc_weighted_backends_reselect_per_concurrent_stream_on_one_downstream_connection() {
    install_rustls_provider();
    let upstream_a = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("upstream a bind");
    let upstream_a_addr = upstream_a.local_addr().expect("upstream a addr");
    let upstream_b = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("upstream b bind");
    let upstream_b_addr = upstream_b.local_addr().expect("upstream b addr");
    let gateway_port = free_tcp_port();
    let snapshot = weighted_mesh_grpc_h2c_snapshot(
        gateway_port,
        upstream_a_addr.port() as u32,
        upstream_b_addr.port() as u32,
    );
    let runtime = RuntimeOptions {
        enable_ipv6: false,
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

    let (seen_tx, mut seen_rx) = mpsc::unbounded_channel::<&'static str>();
    let upstream_a_task = tokio::spawn(serve_grpc_backend_connections(
        upstream_a,
        "a",
        seen_tx.clone(),
    ));
    let upstream_b_task = tokio::spawn(serve_grpc_backend_connections(upstream_b, "b", seen_tx));

    wait_for_listener(gateway_port).await;

    let result = async {
        let stream = TcpStream::connect(("127.0.0.1", gateway_port)).await?;
        let (send_request, connection) = h2client::handshake(stream).await?;
        let connection_task = tokio::spawn(connection);
        let mut send_request = send_request;
        let mut responses = Vec::new();

        for request_id in 0..20 {
            let mut ready = send_request.ready().await?;
            let (response, _) = ready.send_request(
                Request::builder()
                    .method("POST")
                    .uri("http://echo.default.svc.cluster.local/helloworld.Greeter/Watch")
                    .version(http::Version::HTTP_2)
                    .header("content-type", "application/grpc")
                    .header("te", "trailers")
                    .header("x-request-id", request_id.to_string())
                    .body(())?,
                true,
            )?;
            send_request = ready;
            responses.push(response);
        }

        for response in responses {
            let response = response.await?;
            let status = response.status();
            let headers = response.headers().clone();
            assert_eq!(
                status,
                StatusCode::OK,
                "unexpected concurrent mesh gRPC status {status}; headers={headers:?}; traffic={:?}",
                traffic.snapshot()
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
                Some("0")
            );
        }

        drop(send_request);
        connection_task.abort();
        let _ = connection_task.await;
        Ok::<(), anyhow::Error>(())
    }
    .await;

    stop_server(server);
    result.expect("mesh grpc concurrent weighted client flow");

    let mut seen = Vec::new();
    for _ in 0..20 {
        seen.push(
            timeout(Duration::from_secs(2), seen_rx.recv())
                .await
                .expect("backend observation timeout")
                .expect("backend observation missing"),
        );
    }
    seen.sort_unstable();
    seen.dedup();
    assert_eq!(seen, vec!["a", "b"]);

    upstream_a_task
        .await
        .expect("upstream a task")
        .expect("upstream a result");
    upstream_b_task
        .await
        .expect("upstream b task")
        .expect("upstream b result");
}
