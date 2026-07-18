#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn https_misdirected_request_returns_421_before_upstream() {
    install_rustls_provider();
    let upstream_listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("upstream bind");
    let upstream_addr = upstream_listener.local_addr().expect("upstream addr");
    let gateway_port = free_tcp_port();
    let snapshot = https_misdirected_snapshot(
        gateway_port,
        upstream_addr.port() as u32,
        upstream_addr.port(),
    );
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

    let result = https_http1_request(
        gateway_port,
        "example.org",
        "second-example.org",
        "/detect-misdirected-requests",
    )
    .await;

    stop_server(server);
    let response = result.expect("https request");
    assert!(
        response.starts_with("HTTP/1.1 421"),
        "unexpected response: {response}"
    );
    assert!(
        timeout(Duration::from_millis(100), accepted_rx)
            .await
            .is_err(),
        "misdirected request should not connect to upstream"
    );
    upstream
        .await
        .expect("upstream task")
        .expect("upstream result");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn https_listener_negotiates_h2_alpn_with_dynamic_certificate_callback() {
    install_rustls_provider();
    let gateway_port = free_tcp_port();
    let snapshot = https_misdirected_snapshot(gateway_port, 8080, 8081);
    let runtime = RuntimeOptions {
        enable_ipv6: false,
        ..RuntimeOptions::default()
    };
    let plan = build_listener_plan(&snapshot.load(), &runtime, None).expect("plan");
    let server = start_server(
        plan,
        snapshot,
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
    let mut stream = https_tls_connect(gateway_port, "example.org", b"\x02h2")
        .await
        .expect("tls connect");
    assert_eq!(stream.ssl().selected_alpn_protocol(), Some(&b"h2"[..]));

    let _ = stream.shutdown().await;
    stop_server(server);
}

async fn https_http1_request(
    gateway_port: u16,
    sni: &str,
    host: &str,
    path: &str,
) -> anyhow::Result<String> {
    let mut stream = https_tls_connect(gateway_port, sni, b"").await?;
    stream
        .write_all(
            format!("GET {path} HTTP/1.1\r\nHost: {host}\r\nConnection: close\r\n\r\n").as_bytes(),
        )
        .await?;
    read_tls_http_response(&mut stream).await
}

async fn https_tls_connect(
    gateway_port: u16,
    sni: &str,
    alpn_protos: &[u8],
) -> anyhow::Result<SslStream<TcpStream>> {
    let tcp = TcpStream::connect(("127.0.0.1", gateway_port)).await?;
    let mut connector = SslConnector::builder(SslMethod::tls())?;
    connector.set_verify(SslVerifyMode::NONE);
    if !alpn_protos.is_empty() {
        connector.set_alpn_protos(alpn_protos)?;
    }
    let ssl = connector.build().configure()?.into_ssl(sni)?;
    let mut stream = SslStream::new(ssl, tcp)?;
    Pin::new(&mut stream).connect().await?;
    Ok(stream)
}

async fn read_tls_http_response(stream: &mut SslStream<TcpStream>) -> anyhow::Result<String> {
    let mut headers = Vec::new();
    loop {
        let byte = timeout(Duration::from_secs(2), stream.read_u8()).await??;
        headers.push(byte);
        if headers.ends_with(b"\r\n\r\n") {
            break;
        }
    }
    let headers_text = String::from_utf8(headers.clone())?;
    let mut body = Vec::new();
    if let Some(content_length) =
        header_value(&headers_text, "content-length").and_then(|value| value.parse::<usize>().ok())
    {
        body.resize(content_length, 0);
        stream.read_exact(&mut body).await?;
    }
    headers.extend_from_slice(&body);
    Ok(String::from_utf8(headers)?)
}

fn https_misdirected_snapshot(
    listener_port: u16,
    first_backend_port: u32,
    second_backend_port: u16,
) -> ntgw_ir::SharedSnapshot {
    let shared = Snapshot::shared();
    shared.store(Arc::new(Snapshot {
        listeners: vec![
            https_misdirected_listener("default/gw/https", listener_port, vec![]),
            https_misdirected_listener(
                "default/gw/https-with-hostname",
                listener_port,
                vec!["second-example.org"],
            ),
        ],
        http_routes: vec![
            HttpRoute {
                name: "example".to_string(),
                namespace: "default".to_string(),
                hostnames: vec!["example.org".to_string()],
                parent_refs: vec![ParentRef {
                    namespace: "default".to_string(),
                    name: "gw".to_string(),
                    section_name: "https".to_string(),
                    port: listener_port as u32,
                    ..ParentRef::default()
                }],
                rules: vec![https_misdirected_rule(first_backend_port)],
                labels: BTreeMap::new(),
                annotations: BTreeMap::new(),
            },
            HttpRoute {
                name: "second".to_string(),
                namespace: "default".to_string(),
                hostnames: Vec::new(),
                parent_refs: vec![ParentRef {
                    namespace: "default".to_string(),
                    name: "gw".to_string(),
                    section_name: "https-with-hostname".to_string(),
                    port: listener_port as u32,
                    ..ParentRef::default()
                }],
                rules: vec![https_misdirected_rule(second_backend_port as u32)],
                labels: BTreeMap::new(),
                annotations: BTreeMap::new(),
            },
        ],
        backends: vec![
            https_misdirected_backend(first_backend_port),
            https_misdirected_backend(second_backend_port as u32),
        ],
        secrets: vec![SecretMaterial {
            namespace: "default".to_string(),
            name: "example-cert".to_string(),
            cert_pem: VALID_SERVER_CERT_PEM.to_string(),
            key_pem: VALID_SERVER_KEY_PEM.to_string(),
        }],
        ..Snapshot::default()
    }));
    let mut s = (**shared.load()).clone();
    s.rebuild_runtime_indexes();
    shared.store(Arc::new(s));
    shared
}

fn https_misdirected_listener(name: &str, port: u16, hostnames: Vec<&str>) -> Listener {
    Listener {
        name: name.to_string(),
        address: "127.0.0.1".to_string(),
        addresses: vec!["127.0.0.1".to_string()],
        port: port as u32,
        protocol: "LISTENER_PROTOCOL_HTTPS".to_string(),
        hostnames: hostnames.into_iter().map(str::to_string).collect(),
        attached_routes: vec!["default/example".to_string(), "default/second".to_string()],
        tls: Some(TlsConfig {
            enabled: true,
            secret_refs: vec!["default/example-cert".to_string()],
            ..TlsConfig::default()
        }),
        ..Listener::default()
    }
}

fn https_misdirected_rule(backend_port: u32) -> HttpRule {
    HttpRule {
        name: String::new(),
        matches: vec![HttpMatch {
            path: "/detect-misdirected-requests".to_string(),
            path_type: "PathPrefix".to_string(),
            ..HttpMatch::default()
        }],
        backend_refs: vec![BackendRef {
            namespace: "default".to_string(),
            name: "backend".to_string(),
            port: backend_port,
            ..BackendRef::default()
        }],
        ..HttpRule::default()
    }
}

fn https_misdirected_backend(port: u32) -> BackendCluster {
    BackendCluster {
        ai_service: None,
        token_policy: None,
        name: format!("backend:{port}").into(),
        namespace: "default".to_string().into(),
        protocol: "HTTP".to_string().into(),
        endpoints: vec![BackendEndpoint {
            address: "127.0.0.1".to_string(),
            port,
            healthy: true,
        }],
        wasm_plugin: None,
        circuit_breaker: None,
    }
}
