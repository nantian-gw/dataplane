use aeg_ir::{
    BackendCluster, BackendEndpoint, BackendRef, HttpMatch, HttpRoute, HttpRule, ParentRef,
    SecretMaterial, SharedSnapshot, StreamMatch, StreamRoute, StreamRule,
    TlsRouteMode,
};
use tokio::time::{timeout, Duration};

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn shared_tls_terminate_negotiates_h2_alpn() -> Result<()> {
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let addr = listener.local_addr()?;
    let terminate = TerminateSurface {
        listener_names: vec!["default/gw/https".to_string()],
        identities: vec![SharedTlsIdentity {
            secret_ref: "default/server".to_string(),
            cert_pem: SERVER_CERT_PEM.to_string(),
            key_pem: SERVER_KEY_PEM.to_string(),
            match_names: vec!["server-san.example".to_string()],
        }],
        frontend_validation_mode: None,
        client_ca_bundle_pem: None,
    };

    let server = tokio::spawn(async move {
        let (stream, _) = listener.accept().await?;
        let stream = L4Stream::from(stream);
        let mut tls_stream = terminate_tls(stream, &terminate).await?;
        tls_stream.write_all(b"ok").await?;
        Ok::<(), anyhow::Error>(())
    });

    let tcp = tokio::net::TcpStream::connect(addr).await?;
    let mut connector = SslConnector::builder(SslMethod::tls())?;
    connector.set_verify(SslVerifyMode::NONE);
    connector.set_alpn_protos(b"\x02h2")?;
    let ssl = connector
        .build()
        .configure()?
        .into_ssl("server-san.example")?;
    let mut client_stream = SslStream::new(ssl, tcp)?;
    Pin::new(&mut client_stream).connect().await?;
    assert_eq!(client_stream.ssl().selected_alpn_protocol(), Some(&b"h2"[..]));

    let mut response = [0_u8; 2];
    client_stream.read_exact(&mut response).await?;
    assert_eq!(&response, b"ok");
    server.await??;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn shared_tls_runtime_detects_misdirected_https_requests() -> Result<()> {
    let http_backend = TcpListener::bind("127.0.0.1:0").await?;
    let http_backend_addr = http_backend.local_addr()?;
    let stream_backend = TcpListener::bind("127.0.0.1:0").await?;
    let stream_backend_addr = stream_backend.local_addr()?;
    let gateway_port = free_port();
    let bind_addr = format!("127.0.0.1:{gateway_port}");
    let snapshot = shared_tls_misdirected_snapshot(
        gateway_port,
        http_backend_addr.port(),
        stream_backend_addr.port(),
    );
    let plan = build_listener_plan(&snapshot.read(), &RuntimeOptions::default())?;
    let bind = Arc::new(plan.binds.get(&bind_addr).cloned().expect("bind"));
    let gateway_listener = TcpListener::bind(&bind_addr).await?;
    let app = build_http_app(
        snapshot.clone(),
        HttpRuntimeOptions::default(),
        AccessLogOptions {
            enabled: false,
            ..AccessLogOptions::default()
        },
        SessionPersistenceOptions::build(None, None)?,
        SharedTrafficStats::shared(),
        aeg_observability::OverloadStats::shared(),
        HttpCircuitBreakerController::new(Default::default()),
        HttpRateLimitController::new(Default::default()),
        RetryBudgetController::new(Default::default()),
        None,
    )?;
    let (_shutdown_tx, shutdown_rx) = watch::channel(false);

    let upstream_http = tokio::spawn(async move {
        let (mut stream, _) = http_backend.accept().await?;
        let request = read_http_headers_local(&mut stream).await?;
        assert!(request.starts_with("GET /detect-misdirected-requests HTTP/1.1\r\n"));
        stream
            .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok")
            .await?;
        Ok::<(), anyhow::Error>(())
    });
    let upstream_stream = tokio::spawn(async move {
        let (mut stream, _) = stream_backend.accept().await?;
        let mut observed = vec![0; build_client_hello_local("passthrough.example.com").len()];
        stream.read_exact(&mut observed).await?;
        stream.write_all(b"stream-backend").await?;
        stream.shutdown().await?;
        Ok::<(), anyhow::Error>(())
    });

    let server = tokio::spawn(async move {
        loop {
            let (stream, _) = gateway_listener.accept().await?;
            let bind = bind.clone();
            let snapshot = snapshot.clone();
            let app = app.clone();
            let shutdown_rx = shutdown_rx.clone();
            tokio::spawn(async move {
                let _ = crate::runtime::handle_connection(
                    bind,
                    stream,
                    snapshot,
                    app,
                    shutdown_rx,
                    ConnectionConfig,
                )
                .await;
            });
        }
        #[allow(unreachable_code)]
        Ok::<(), anyhow::Error>(())
    });

    let response = https_request_local(
        &bind_addr,
        "example.org",
        "second-example.org",
        "/detect-misdirected-requests",
    )
    .await?;

    server.abort();
    upstream_http.abort();
    upstream_stream.abort();

    assert!(
        response.starts_with("HTTP/1.1 421"),
        "unexpected response: {response}"
    );
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn shared_tls_rejects_invalid_frontend_validation_after_listener_selection() -> Result<()> {
    let http_backend = TcpListener::bind("127.0.0.1:0").await?;
    let http_backend_addr = http_backend.local_addr()?;
    let gateway_port = free_port();
    let bind_addr = format!("127.0.0.1:{gateway_port}");
    let snapshot =
        shared_tls_frontend_validation_snapshot(gateway_port, http_backend_addr.port());
    let plan = build_listener_plan(&snapshot.read(), &RuntimeOptions::default())?;
    let bind = Arc::new(plan.binds.get(&bind_addr).cloned().expect("bind"));
    let gateway_listener = TcpListener::bind(&bind_addr).await?;
    let app = build_http_app(
        snapshot.clone(),
        HttpRuntimeOptions::default(),
        AccessLogOptions {
            enabled: false,
            ..AccessLogOptions::default()
        },
        SessionPersistenceOptions::build(None, None)?,
        SharedTrafficStats::shared(),
        aeg_observability::OverloadStats::shared(),
        HttpCircuitBreakerController::new(Default::default()),
        HttpRateLimitController::new(Default::default()),
        RetryBudgetController::new(Default::default()),
        None,
    )?;
    let (_shutdown_tx, shutdown_rx) = watch::channel(false);
    let (request_tx, mut request_rx) = tokio::sync::mpsc::unbounded_channel();

    let upstream_http = tokio::spawn(async move {
        loop {
            let Ok(Ok((mut stream, _))) =
                timeout(Duration::from_millis(800), http_backend.accept()).await
            else {
                break;
            };
            let request = read_http_headers_local(&mut stream).await?;
            let _ = request_tx.send(request);
            stream
                .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok")
                .await?;
        }
        Ok::<(), anyhow::Error>(())
    });

    let server = tokio::spawn(async move {
        loop {
            let (stream, _) = gateway_listener.accept().await?;
            let bind = bind.clone();
            let snapshot = snapshot.clone();
            let app = app.clone();
            let shutdown_rx = shutdown_rx.clone();
            tokio::spawn(async move {
                let _ = crate::runtime::handle_connection(
                    bind,
                    stream,
                    snapshot,
                    app,
                    shutdown_rx,
                    ConnectionConfig,
                )
                .await;
            });
        }
        #[allow(unreachable_code)]
        Ok::<(), anyhow::Error>(())
    });

    let valid_response = https_request_local(&bind_addr, "good.example", "good.example", "/")
        .await?;
    assert!(
        valid_response.starts_with("HTTP/1.1 200"),
        "unexpected valid listener response: {valid_response}"
    );
    let valid_request = timeout(Duration::from_secs(2), request_rx.recv())
        .await?
        .expect("valid upstream request");
    assert!(
        valid_request.contains("Host: good.example"),
        "unexpected valid upstream request: {valid_request}"
    );

    let rejected = https_request_local(&bind_addr, "bad.example", "bad.example", "/").await;

    server.abort();
    upstream_http.abort();

    assert!(
        rejected.is_err(),
        "invalid frontend validation listener should close the request, got {rejected:?}"
    );
    match timeout(Duration::from_millis(150), request_rx.recv()).await {
        Err(_) => {}
        Ok(Some(request)) => panic!(
            "invalid frontend validation rejection should not connect to upstream, got {request}"
        ),
        Ok(None) => {}
    }
    Ok(())
}

fn build_client_hello_local(host: &str) -> Vec<u8> {
    let mut body = Vec::new();
    body.extend_from_slice(&[0x03, 0x03]);
    body.extend_from_slice(&[0; 32]);
    body.push(0);
    body.extend_from_slice(&[0x00, 0x02, 0x00, 0x2f]);
    body.extend_from_slice(&[0x01, 0x00]);

    let mut server_name = Vec::new();
    server_name.extend_from_slice(&(host.len() as u16 + 3).to_be_bytes());
    server_name.push(0);
    server_name.extend_from_slice(&(host.len() as u16).to_be_bytes());
    server_name.extend_from_slice(host.as_bytes());

    let mut extensions = Vec::new();
    extensions.extend_from_slice(&[0x00, 0x00]);
    extensions.extend_from_slice(&(server_name.len() as u16).to_be_bytes());
    extensions.extend_from_slice(&server_name);

    body.extend_from_slice(&(extensions.len() as u16).to_be_bytes());
    body.extend_from_slice(&extensions);

    let mut handshake = vec![
        0x01,
        ((body.len() >> 16) & 0xff) as u8,
        ((body.len() >> 8) & 0xff) as u8,
        (body.len() & 0xff) as u8,
    ];
    handshake.extend_from_slice(&body);

    let mut record = Vec::new();
    record.extend_from_slice(&[0x16, 0x03, 0x01]);
    record.extend_from_slice(&(handshake.len() as u16).to_be_bytes());
    record.extend_from_slice(&handshake);
    record
}

async fn read_http_headers_local<S>(stream: &mut S) -> Result<String>
where
    S: tokio::io::AsyncRead + Unpin,
{
    let mut buf = Vec::new();
    loop {
        let byte = timeout(Duration::from_secs(2), stream.read_u8()).await??;
        buf.push(byte);
        if buf.ends_with(b"\r\n\r\n") {
            break;
        }
    }
    Ok(String::from_utf8(buf)?)
}

async fn read_http_response_local<S>(stream: &mut S) -> Result<String>
where
    S: tokio::io::AsyncRead + Unpin,
{
    let headers = read_http_headers_local(stream).await?;
    let content_length = headers
        .lines()
        .find_map(|line| {
            let (name, value) = line.split_once(':')?;
            name.trim()
                .eq_ignore_ascii_case("content-length")
                .then(|| value.trim().parse::<usize>().ok())
                .flatten()
        })
        .unwrap_or_default();
    let mut body = vec![0; content_length];
    if content_length > 0 {
        stream.read_exact(&mut body).await?;
    }
    let mut raw = headers.into_bytes();
    raw.extend_from_slice(&body);
    Ok(String::from_utf8(raw)?)
}

async fn https_request_local(bind: &str, sni: &str, host: &str, path: &str) -> Result<String> {
    let tcp = tokio::net::TcpStream::connect(bind).await?;
    let mut connector = SslConnector::builder(SslMethod::tls())?;
    connector.set_verify(SslVerifyMode::NONE);
    let ssl = connector
        .build()
        .configure()?
        .into_ssl(sni)?;
    let mut stream = SslStream::new(ssl, tcp)?;
    Pin::new(&mut stream).connect().await?;
    stream
        .write_all(
            format!("GET {path} HTTP/1.1\r\nHost: {host}\r\nConnection: close\r\n\r\n").as_bytes(),
        )
        .await?;
    read_http_response_local(&mut stream).await
}

fn shared_tls_frontend_validation_snapshot(
    gateway_port: u16,
    http_backend_port: u16,
) -> SharedSnapshot {
    let shared = Snapshot::shared();
    *shared.write() = Snapshot {
        listeners: vec![
            Listener {
                name: "default/gw/good".to_string(),
                address: "127.0.0.1".to_string(),
                addresses: vec!["127.0.0.1".to_string()],
                port: gateway_port as u32,
                protocol: "LISTENER_PROTOCOL_HTTPS".to_string(),
                hostnames: vec!["good.example".to_string()],
                attached_routes: vec!["default/good".to_string()],
                tls: Some(TlsConfig {
                    enabled: true,
                    passthrough: false,
                    secret_refs: vec!["default/example-cert".to_string()],
                    sni_hosts: vec!["good.example".to_string()],
                    min_version: "1.2".to_string(),
                    max_version: "1.3".to_string(),
                    frontend_validation: None,
                }),
                ..Listener::default()
            },
            Listener {
                name: "default/gw/bad".to_string(),
                address: "127.0.0.1".to_string(),
                addresses: vec!["127.0.0.1".to_string()],
                port: gateway_port as u32,
                protocol: "LISTENER_PROTOCOL_HTTPS".to_string(),
                hostnames: vec!["bad.example".to_string()],
                attached_routes: vec!["default/bad".to_string()],
                tls: Some(TlsConfig {
                    enabled: true,
                    passthrough: false,
                    secret_refs: vec!["default/example-cert".to_string()],
                    sni_hosts: vec!["bad.example".to_string()],
                    min_version: "1.2".to_string(),
                    max_version: "1.3".to_string(),
                    frontend_validation: Some(aeg_ir::FrontendValidation {
                        ca_pems: Vec::new(),
                        mode: "RejectClientCertificate".to_string(),
                    }),
                }),
                ..Listener::default()
            },
        ],
        http_routes: vec![
            HttpRoute {
                name: "good".to_string(),
                namespace: "default".to_string(),
                hostnames: vec!["good.example".to_string()],
                parent_refs: vec![ParentRef {
                    namespace: "default".to_string(),
                    name: "gw".to_string(),
                    section_name: "good".to_string(),
                    port: gateway_port as u32,
                    ..ParentRef::default()
                }],
                rules: vec![HttpRule {
                    name: String::new(),
                    matches: vec![HttpMatch {
                        path: "/".to_string(),
                        path_type: "Exact".to_string(),
                        ..HttpMatch::default()
                    }],
                    backend_refs: vec![BackendRef {
                        namespace: "default".to_string(),
                        name: "http-backend".to_string(),
                        port: http_backend_port as u32,
                        ..BackendRef::default()
                    }],
                    ..HttpRule::default()
                }],
                annotations: BTreeMap::new(),
            },
            HttpRoute {
                name: "bad".to_string(),
                namespace: "default".to_string(),
                hostnames: vec!["bad.example".to_string()],
                parent_refs: vec![ParentRef {
                    namespace: "default".to_string(),
                    name: "gw".to_string(),
                    section_name: "bad".to_string(),
                    port: gateway_port as u32,
                    ..ParentRef::default()
                }],
                rules: vec![HttpRule {
                    name: String::new(),
                    matches: vec![HttpMatch {
                        path: "/".to_string(),
                        path_type: "Exact".to_string(),
                        ..HttpMatch::default()
                    }],
                    backend_refs: vec![BackendRef {
                        namespace: "default".to_string(),
                        name: "http-backend".to_string(),
                        port: http_backend_port as u32,
                        ..BackendRef::default()
                    }],
                    ..HttpRule::default()
                }],
                annotations: BTreeMap::new(),
            },
        ],
        backends: vec![BackendCluster {
            ai_service: None,
            token_policy: None,
            name: format!("http-backend:{http_backend_port}"),
            namespace: "default".to_string(),
            protocol: "HTTP".to_string(),
            endpoints: vec![BackendEndpoint {
                address: "127.0.0.1".to_string(),
                port: http_backend_port as u32,
                healthy: true,
            }],
            wasm_plugin: None,
        }],
        secrets: vec![SecretMaterial {
            namespace: "default".to_string(),
            name: "example-cert".to_string(),
            cert_pem: SERVER_CERT_PEM.to_string(),
            key_pem: SERVER_KEY_PEM.to_string(),
        }],
        ..Snapshot::default()
    };
    shared.write().rebuild_runtime_indexes();
    shared
}

fn shared_tls_misdirected_snapshot(
    gateway_port: u16,
    http_backend_port: u16,
    stream_backend_port: u16,
) -> SharedSnapshot {
    let shared = Snapshot::shared();
    *shared.write() = Snapshot {
        listeners: vec![
            Listener {
                name: "default/gw/https".to_string(),
                address: "127.0.0.1".to_string(),
                addresses: vec!["127.0.0.1".to_string()],
                port: gateway_port as u32,
                protocol: "LISTENER_PROTOCOL_HTTPS".to_string(),
                attached_routes: vec!["default/example".to_string()],
                tls: Some(TlsConfig {
                    enabled: true,
                    passthrough: false,
                    secret_refs: vec!["default/example-cert".to_string()],
                    sni_hosts: vec![],
                    min_version: "1.2".to_string(),
                    max_version: "1.3".to_string(),
                    frontend_validation: None,
                }),
                ..Listener::default()
            },
            Listener {
                name: "default/gw/https-with-hostname".to_string(),
                address: "127.0.0.1".to_string(),
                addresses: vec!["127.0.0.1".to_string()],
                port: gateway_port as u32,
                protocol: "LISTENER_PROTOCOL_HTTPS".to_string(),
                hostnames: vec!["second-example.org".to_string()],
                attached_routes: vec!["default/second".to_string()],
                tls: Some(TlsConfig {
                    enabled: true,
                    passthrough: false,
                    secret_refs: vec!["default/example-cert".to_string()],
                    sni_hosts: vec!["second-example.org".to_string()],
                    min_version: "1.2".to_string(),
                    max_version: "1.3".to_string(),
                    frontend_validation: None,
                }),
                ..Listener::default()
            },
            Listener {
                name: "default/gw/tls".to_string(),
                address: "127.0.0.1".to_string(),
                addresses: vec!["127.0.0.1".to_string()],
                port: gateway_port as u32,
                protocol: "LISTENER_PROTOCOL_TLS_PASSTHROUGH".to_string(),
                attached_routes: vec!["default/tls-route".to_string()],
                tls: Some(TlsConfig {
                    enabled: true,
                    passthrough: true,
                    secret_refs: vec![],
                    sni_hosts: vec![],
                    min_version: String::new(),
                    max_version: String::new(),
                    frontend_validation: None,
                }),
                ..Listener::default()
            },
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
                    port: gateway_port as u32,
                    ..ParentRef::default()
                }],
                rules: vec![HttpRule {
                    name: String::new(),
                    matches: vec![HttpMatch {
                        path: "/detect-misdirected-requests".to_string(),
                        path_type: "Exact".to_string(),
                        ..HttpMatch::default()
                    }],
                    backend_refs: vec![BackendRef {
                        namespace: "default".to_string(),
                        name: "http-backend".to_string(),
                        port: http_backend_port as u32,
                        ..BackendRef::default()
                    }],
                    ..HttpRule::default()
                }],
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
                    port: gateway_port as u32,
                    ..ParentRef::default()
                }],
                rules: vec![HttpRule {
                    name: String::new(),
                    matches: vec![HttpMatch {
                        path: "/detect-misdirected-requests".to_string(),
                        path_type: "Exact".to_string(),
                        ..HttpMatch::default()
                    }],
                    backend_refs: vec![BackendRef {
                        namespace: "default".to_string(),
                        name: "http-backend".to_string(),
                        port: http_backend_port as u32,
                        ..BackendRef::default()
                    }],
                    ..HttpRule::default()
                }],
                annotations: BTreeMap::new(),
            },
        ],
        stream_routes: vec![StreamRoute {
            name: "tls-route".to_string(),
            namespace: "default".to_string(),
            kind: "ROUTE_KIND_TLS".to_string(),
            parent_refs: Vec::new(),
            rules: vec![StreamRule {
                name: String::new(),
                matches: vec![StreamMatch {
                    port: gateway_port as u32,
                    sni_hostname: "passthrough.example.com".to_string(),
                    mode: TlsRouteMode::default(),
                }],
                backend_refs: vec![BackendRef {
                    namespace: "default".to_string(),
                    name: "stream-backend".to_string(),
                    port: stream_backend_port as u32,
                    ..BackendRef::default()
                }],
            }],
            annotations: BTreeMap::new(),
        }],
        backends: vec![
            BackendCluster {
                ai_service: None,
                token_policy: None,
                name: format!("http-backend:{http_backend_port}"),
                namespace: "default".to_string(),
                protocol: "HTTP".to_string(),
                endpoints: vec![BackendEndpoint {
                    address: "127.0.0.1".to_string(),
                    port: http_backend_port as u32,
                    healthy: true,
                }],
                wasm_plugin: None,
            },
            BackendCluster {
                ai_service: None,
                token_policy: None,
                name: format!("stream-backend:{stream_backend_port}"),
                namespace: "default".to_string(),
                protocol: "TCP".to_string(),
                endpoints: vec![BackendEndpoint {
                    address: "127.0.0.1".to_string(),
                    port: stream_backend_port as u32,
                    healthy: true,
                }],
                wasm_plugin: None,
            },
        ],
        secrets: vec![SecretMaterial {
            namespace: "default".to_string(),
            name: "example-cert".to_string(),
            cert_pem: SERVER_CERT_PEM.to_string(),
            key_pem: SERVER_KEY_PEM.to_string(),
        }],
        ..Snapshot::default()
    };
    shared.write().rebuild_runtime_indexes();
    shared
}
