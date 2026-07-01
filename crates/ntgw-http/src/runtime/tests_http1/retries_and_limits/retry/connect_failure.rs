#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn retry_after_connect_failure_reselects_next_backend_and_records_retry_metrics() {
    install_rustls_provider();
    let healthy_listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("healthy backend bind");
    let healthy_addr = healthy_listener.local_addr().expect("healthy addr");
    let failing_port = free_tcp_port();
    let gateway_port = free_tcp_port();
    let retry = RetryPolicy {
        codes: vec![503],
        attempts: 2,
        backoff: None,
    };
    let snapshot = multi_backend_http_snapshot(
        gateway_port,
        "/retry-connect",
        &[
            ("failing", failing_port as u32, "HTTP", 1),
            ("healthy", healthy_addr.port() as u32, "HTTP", 1),
        ],
        Some(retry),
    );
    let runtime = RuntimeOptions {
        enable_ipv6: false,
        ..RuntimeOptions::default()
    };
    let plan = build_listener_plan(&snapshot.load(), &runtime, None).expect("plan");
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

    let healthy = tokio::spawn(async move {
        let (mut stream, _) = healthy_listener.accept().await?;
        let request = read_http_headers(&mut stream).await?;
        assert!(request.starts_with("GET /retry-connect HTTP/1.1\r\n"));
        stream
            .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nok")
            .await?;
        Ok::<(), anyhow::Error>(())
    });

    wait_for_listener(gateway_port).await;

    let result = async {
        let mut client = TcpStream::connect(("127.0.0.1", gateway_port)).await?;
        client
            .write_all(b"GET /retry-connect HTTP/1.1\r\nHost: example.com\r\n\r\n")
            .await?;
        let response = read_http_response(&mut client).await?;
        assert!(response.starts_with("HTTP/1.1 200"));
        assert!(response.ends_with("\r\n\r\nok"));
        Ok::<(), anyhow::Error>(())
    }
    .await;

    stop_server(server);
    result.expect("retry connect client flow");
    healthy
        .await
        .expect("healthy task")
        .expect("healthy result");

    let stats = wait_for_traffic_snapshot(&traffic, |stats| {
        stats.total_events == 1 && stats.total_retry_attempts == 1 && stats.status_2xx == 1
    })
    .await;
    assert_eq!(stats.total_events, 1);
    assert_eq!(stats.total_retried_events, 1);
    assert_eq!(stats.total_retry_attempts, 1);
    assert_eq!(stats.status_2xx, 1);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn default_transport_retry_after_connect_failure_reselects_fast_path_backend() {
    install_rustls_provider();
    let healthy_listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("healthy backend bind");
    let healthy_addr = healthy_listener.local_addr().expect("healthy addr");
    let stale_port = free_tcp_port();
    let gateway_port = free_tcp_port();
    let snapshot = multi_backend_http_snapshot(
        gateway_port,
        "/retry-connect-default",
        &[
            ("stale", stale_port as u32, "HTTP", 1),
            ("healthy", healthy_addr.port() as u32, "HTTP", 1),
        ],
        None,
    );
    let runtime = RuntimeOptions {
        enable_ipv6: false,
        ..RuntimeOptions::default()
    };
    let plan = build_listener_plan(&snapshot.load(), &runtime, None).expect("plan");
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

    let healthy = tokio::spawn(async move {
        let (mut stream, _) = healthy_listener.accept().await?;
        let request = read_http_headers(&mut stream).await?;
        assert!(request.starts_with(
            "GET /retry-connect-default HTTP/1.1\r\n"
        ));
        stream
            .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nok")
            .await?;
        Ok::<(), anyhow::Error>(())
    });

    wait_for_listener(gateway_port).await;

    let result = async {
        let mut client = TcpStream::connect(("127.0.0.1", gateway_port)).await?;
        client
            .write_all(b"GET /retry-connect-default HTTP/1.1\r\nHost: example.com\r\n\r\n")
            .await?;
        let response = read_http_response(&mut client).await?;
        assert!(
            response.starts_with("HTTP/1.1 200"),
            "transport connect failure should retry to healthy backend, got: {response}"
        );
        assert!(response.ends_with("\r\n\r\nok"));
        Ok::<(), anyhow::Error>(())
    }
    .await;

    stop_server(server);
    result.expect("default transport retry client flow");
    healthy
        .await
        .expect("healthy task")
        .expect("healthy result");

    let stats = wait_for_traffic_snapshot(&traffic, |stats| {
        stats.total_events == 1 && stats.total_retry_attempts == 1 && stats.status_2xx == 1
    })
    .await;
    assert_eq!(stats.total_events, 1);
    assert_eq!(stats.total_retried_events, 1);
    assert_eq!(stats.total_retry_attempts, 1);
    assert_eq!(stats.status_2xx, 1);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn default_transport_retry_avoids_failed_endpoint_for_concurrent_fast_path_requests() {
    install_rustls_provider();
    let healthy_listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("healthy backend bind");
    let healthy_addr = healthy_listener.local_addr().expect("healthy addr");
    let stale_port = free_tcp_port();
    let gateway_port = free_tcp_port();
    let request_count = 8usize;
    let snapshot = multi_backend_http_snapshot(
        gateway_port,
        "/retry-connect-concurrent",
        &[
            ("stale", stale_port as u32, "HTTP", 1),
            ("healthy", healthy_addr.port() as u32, "HTTP", 1),
        ],
        None,
    );
    let runtime = RuntimeOptions {
        enable_ipv6: false,
        ..RuntimeOptions::default()
    };
    let plan = build_listener_plan(&snapshot.load(), &runtime, None).expect("plan");
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

    let accepted = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let healthy_accepted = accepted.clone();
    let (healthy_shutdown_tx, mut healthy_shutdown_rx) = oneshot::channel::<()>();
    let healthy = tokio::spawn(async move {
        loop {
            tokio::select! {
                _ = &mut healthy_shutdown_rx => break,
                accepted = healthy_listener.accept() => {
                    let (mut stream, _) = accepted?;
                    healthy_accepted.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                    tokio::spawn(async move {
                        let request = read_http_headers(&mut stream).await?;
                        assert!(request.starts_with(
                            "GET /retry-connect-concurrent HTTP/1.1\r\n"
                        ));
                        stream
                            .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nok")
                            .await?;
                        Ok::<(), anyhow::Error>(())
                    });
                }
            }
        }
        Ok::<(), anyhow::Error>(())
    });

    wait_for_listener(gateway_port).await;

    let mut clients = Vec::with_capacity(request_count);
    for _ in 0..request_count {
        clients.push(tokio::spawn(async move {
            let mut client = TcpStream::connect(("127.0.0.1", gateway_port)).await?;
            client
                .write_all(
                    b"GET /retry-connect-concurrent HTTP/1.1\r\nHost: example.com\r\n\r\n",
                )
                .await?;
            read_http_response(&mut client).await
        }));
    }

    let mut unexpected = Vec::new();
    for client in clients {
        let response = client.await.expect("client task").expect("client flow");
        if !response.starts_with("HTTP/1.1 200") {
            unexpected.push(response);
        }
    }

    stop_server(server);
    let _ = healthy_shutdown_tx.send(());
    healthy.await.expect("healthy task").expect("healthy result");

    assert!(
        unexpected.is_empty(),
        "transport retry should not select a just-failed endpoint again, got: {unexpected:#?}"
    );
    assert!(
        accepted.load(std::sync::atomic::Ordering::SeqCst) >= request_count,
        "healthy backend should receive all original healthy selections and successful retries"
    );

    let stats = wait_for_traffic_snapshot(&traffic, |stats| {
        stats.total_events == request_count as u64 && stats.status_5xx == 0
    })
    .await;
    assert_eq!(stats.total_events, request_count as u64);
    assert_eq!(stats.status_5xx, 0);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn no_healthy_backend_fast_fails_and_emits_error_headers_in_access_log() {
    install_rustls_provider();
    let gateway_port = free_tcp_port();
    let backend_port = free_tcp_port() as u32;
    let snapshot = unhealthy_backend_http_snapshot(
        gateway_port,
        "/no-healthy-backend",
        backend_port,
    );
    let runtime = RuntimeOptions {
        enable_ipv6: false,
        ..RuntimeOptions::default()
    };
    let log_path = temp_log_path("no-healthy-backend-access-log");
    let traffic = SharedTrafficStats::shared();
    let server = start_server(
        build_listener_plan(&snapshot.load(), &runtime, None).expect("plan"),
        snapshot.clone(),
        runtime,
        AccessLogOptions {
            enabled: true,
            path: log_path.display().to_string(),
            mode: ntgw_observability::AccessLogMode::Text,
            format: "$status|$sent_http_server|$sent_http_cache_control|$sent_http_content_length"
                .to_string(),
            ..AccessLogOptions::default()
        },
        SessionPersistenceOptions::build(None, None).expect("session options"),
        traffic.clone(),
    )
    .expect("start server");

    wait_for_listener(gateway_port).await;

    let result = async {
        let mut client = TcpStream::connect(("127.0.0.1", gateway_port)).await?;
        client
            .write_all(b"GET /no-healthy-backend HTTP/1.1\r\nHost: example.com\r\n\r\n")
            .await?;
        let response = read_http_response(&mut client).await?;
        assert!(
            response.starts_with("HTTP/1.1 503"),
            "expected 503 response, got: {response}"
        );
        Ok::<(), anyhow::Error>(())
    }
    .await;

    stop_server(server);
    result.expect("no healthy backend client flow");

    let stats = wait_for_traffic_snapshot(&traffic, |stats| {
        stats.total_events == 1
            && stats.status_5xx == 1
            && stats.response_flags.get("UH").copied() == Some(1)
    })
    .await;
    assert_eq!(stats.total_events, 1);
    assert_eq!(stats.status_5xx, 1);
    assert_eq!(stats.total_retry_attempts, 0);
    assert_eq!(stats.total_retried_events, 0);
    assert_eq!(stats.response_flags.get("UH").copied(), Some(1));

    let log_contents = wait_for_log_contents(&log_path).await;
    let line = log_contents
        .lines()
        .find(|line| line.starts_with("503|"))
        .expect("503 access-log line");
    let parts: Vec<_> = line.split('|').collect();
    assert_eq!(parts.len(), 4, "expected four pipe-separated fields");
    assert_ne!(parts[1], "-", "server header should be captured");
    assert_eq!(parts[2], "private, no-store");
    assert_eq!(parts[3], "0");

    shutdown_access_log_writer(&log_path.display().to_string());
    let _ = fs::remove_file(log_path);
}

fn unhealthy_backend_http_snapshot(
    listener_port: u16,
    path: &str,
    backend_port: u32,
) -> ntgw_ir::SharedSnapshot {
    let shared = Snapshot::shared();
    shared.store(Arc::new(Snapshot {
        listeners: vec![Listener {
            name: "default/gw/http".to_string().into(),
            address: "127.0.0.1".to_string(),
            addresses: vec!["127.0.0.1".to_string()],
            port: listener_port as u32,
            protocol: "LISTENER_PROTOCOL_HTTP".to_string().into(),
            attached_routes: vec!["default/route".to_string()],
            ..Listener::default()
        }],
        http_routes: vec![HttpRoute {
            name: "route".to_string().into(),
            namespace: "default".to_string().into(),
            hostnames: Vec::new(),
            parent_refs: vec![ParentRef {
                namespace: "default".to_string().into(),
                name: "gw".to_string().into(),
                section_name: String::new(),
                port: listener_port as u32,
                ..ParentRef::default()
            }],
            rules: vec![HttpRule {
                name: String::new(),
                matches: vec![HttpMatch {
                    path: path.to_string(),
                    path_type: "Exact".to_string(),
                    ..HttpMatch::default()
                }],
                backend_refs: vec![BackendRef {
                    namespace: "default".to_string().into(),
                    name: "backend".to_string().into(),
                    port: backend_port,
                    ..BackendRef::default()
                }],
                ..HttpRule::default()
            }],
            labels: BTreeMap::new(),
            annotations: BTreeMap::new(),
        }],
        backends: vec![BackendCluster {
            ai_service: None,
            token_policy: None,
            name: format!("backend:{backend_port}").into(),
            namespace: "default".to_string().into(),
            protocol: "HTTP".to_string().into(),
            endpoints: vec![BackendEndpoint {
                address: "127.0.0.1".to_string(),
                port: backend_port,
                healthy: false,
            }],
            wasm_plugin: None,
        
                circuit_breaker: None,}],
        ..Snapshot::default()
    }));
    let mut s = (**shared.load()).clone();
    s.rebuild_runtime_indexes();
    shared.store(Arc::new(s));
    shared
}
