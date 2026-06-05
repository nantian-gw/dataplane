#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn strict_frontend_validation_closes_request_when_tls_acceptor_has_not_reloaded_yet() {
    install_rustls_provider();
    let upstream_listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("upstream bind");
    let upstream_addr = upstream_listener.local_addr().expect("upstream addr");
    let gateway_port = free_tcp_port();
    let snapshot =
        https_misdirected_snapshot(gateway_port, upstream_addr.port() as u32, upstream_addr.port());
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

    let (accepted_tx, accepted_rx) = oneshot::channel();
    let upstream = tokio::spawn(async move {
        if let Ok(Ok((mut stream, _))) =
            timeout(Duration::from_millis(500), upstream_listener.accept()).await
        {
            let _ = accepted_tx.send(());
            let _ = read_http_headers(&mut stream).await?;
            stream
                .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nok")
                .await?;
        }
        Ok::<(), anyhow::Error>(())
    });

    wait_for_listener(gateway_port).await;
    {
        let mut current = snapshot.write();
        let tls = current.listeners[0]
            .tls
            .as_mut()
            .expect("https listener tls config");
        tls.frontend_validation = Some(aeg_ir::FrontendValidation {
            ca_pems: vec![VALID_SERVER_CERT_PEM.to_string()],
            mode: "RequireClientCertificate".to_string(),
        });
        current.rebuild_runtime_indexes();
    }

    let result = https_http1_request(
        gateway_port,
        "example.org",
        "example.org",
        "/detect-misdirected-requests",
    )
    .await;

    stop_server(server);
    assert!(
        result.is_err(),
        "request without client certificate should be closed, got {result:?}"
    );
    assert!(
        timeout(Duration::from_millis(100), accepted_rx)
            .await
            .is_err(),
        "frontend validation rejection should not connect to upstream"
    );
    upstream
        .await
        .expect("upstream task")
        .expect("upstream result");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn strict_frontend_validation_rejects_non_matching_client_certificate() {
    install_rustls_provider();
    const CLIENT_CERT_PEM: &str = include_str!("../../../../../testdata/tls/client.crt");
    const CLIENT_KEY_PEM: &str = include_str!("../../../../../testdata/tls/client.key");

    let upstream_listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("upstream bind");
    let upstream_addr = upstream_listener.local_addr().expect("upstream addr");
    let gateway_port = free_tcp_port();
    let snapshot =
        https_misdirected_snapshot(gateway_port, upstream_addr.port() as u32, upstream_addr.port());
    {
        let mut current = snapshot.write();
        let tls = current.listeners[0]
            .tls
            .as_mut()
            .expect("https listener tls config");
        tls.frontend_validation = Some(aeg_ir::FrontendValidation {
            ca_pems: vec![VALID_SERVER_CERT_PEM.to_string()],
            mode: "RequireClientCertificate".to_string(),
        });
        current.rebuild_runtime_indexes();
    }
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

    let (accepted_tx, accepted_rx) = oneshot::channel();
    let upstream = tokio::spawn(async move {
        if let Ok(Ok((mut stream, _))) =
            timeout(Duration::from_millis(500), upstream_listener.accept()).await
        {
            let _ = accepted_tx.send(());
            let _ = read_http_headers(&mut stream).await?;
            stream
                .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nok")
                .await?;
        }
        Ok::<(), anyhow::Error>(())
    });

    wait_for_listener(gateway_port).await;
    let result = https_http1_request_with_client_certificate(
        gateway_port,
        "example.org",
        "example.org",
        "/detect-misdirected-requests",
        CLIENT_CERT_PEM,
        CLIENT_KEY_PEM,
    )
    .await;

    stop_server(server);
    assert!(
        result.is_err(),
        "strict frontend validation should reject a client cert signed by another CA, got {result:?}"
    );
    assert!(
        timeout(Duration::from_millis(100), accepted_rx)
            .await
            .is_err(),
        "rejected frontend client certificate should not connect to upstream"
    );
    upstream
        .await
        .expect("upstream task")
        .expect("upstream result");
}

async fn https_http1_request_with_client_certificate(
    gateway_port: u16,
    sni: &str,
    host: &str,
    path: &str,
    client_cert_pem: &str,
    client_key_pem: &str,
) -> anyhow::Result<String> {
    let tcp = TcpStream::connect(("127.0.0.1", gateway_port)).await?;
    let mut connector = SslConnector::builder(SslMethod::tls())?;
    connector.set_verify(SslVerifyMode::NONE);
    let cert = pingora::tls::x509::X509::from_pem(client_cert_pem.as_bytes())?;
    let key = pingora::tls::pkey::PKey::private_key_from_pem(client_key_pem.as_bytes())?;
    connector.set_certificate(&cert)?;
    connector.set_private_key(&key)?;
    connector.check_private_key()?;
    let ssl = connector.build().configure()?.into_ssl(sni)?;
    let mut stream = SslStream::new(ssl, tcp)?;
    Pin::new(&mut stream).connect().await?;
    stream
        .write_all(
            format!("GET {path} HTTP/1.1\r\nHost: {host}\r\nConnection: close\r\n\r\n")
                .as_bytes(),
        )
        .await?;
    read_tls_http_response(&mut stream).await
}
