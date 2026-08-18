use std::net::SocketAddr;

use ntgw_proto::{
    envoy::{
        service::auth::v3::{
            CheckRequest, CheckResponse, DeniedHttpResponse, OkHttpResponse,
            authorization_server::{Authorization, AuthorizationServer},
            check_response::HttpResponse,
        },
        r#type::v3::{HttpStatus, StatusCode as EnvoyStatusCode},
    },
    google::rpc::Status,
};
use tonic::{
    Request as TonicRequest, Response as TonicResponse, Status as TonicStatus,
    transport::Server as TonicServer,
};

#[derive(Clone)]
struct TestGrpcAuth {
    response: Result<CheckResponse, TonicStatus>,
    observed: tokio::sync::mpsc::Sender<CheckRequest>,
}

static GRPC_AUTH_TEST_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

#[tonic::async_trait]
impl Authorization for TestGrpcAuth {
    async fn check(
        &self,
        request: TonicRequest<CheckRequest>,
    ) -> Result<TonicResponse<CheckResponse>, TonicStatus> {
        self.observed
            .send(request.into_inner())
            .await
            .map_err(|_| TonicStatus::internal("observer dropped"))?;
        self.response.clone().map(TonicResponse::new)
    }
}

fn external_auth_http_snapshot(
    listener_port: u16,
    backend_port: u32,
    auth_port: u32,
    allowed_headers: Vec<&str>,
    allowed_response_headers: Vec<&str>,
) -> ntgw_ir::SharedSnapshot {
    let shared = Snapshot::shared();
    shared.store(Arc::new(Snapshot {
        listeners: vec![Listener {
            name: "default/gw/http".to_string(),
            address: "127.0.0.1".to_string(),
            addresses: vec!["127.0.0.1".to_string()],
            port: listener_port as u32,
            protocol: "LISTENER_PROTOCOL_HTTP".to_string(),
            attached_routes: vec!["default/route".to_string()],
            ..Listener::default()
        }],
        http_routes: vec![HttpRoute {
            name: "route".to_string(),
            namespace: "default".to_string(),
            hostnames: Vec::new(),
            parent_refs: vec![ParentRef {
                namespace: "default".to_string(),
                name: "gw".to_string(),
                section_name: String::new(),
                port: listener_port as u32,
                ..ParentRef::default()
            }],
            rules: vec![HttpRule {
                name: String::new(),
                matches: vec![HttpMatch {
                    path: "/protected".to_string(),
                    path_type: "Exact".to_string(),
                    ..HttpMatch::default()
                }],
                filters: vec![Filter {
                    filter_type: "ExternalAuth".to_string(),
                    external_auth: Some(ntgw_ir::ExternalAuthFilter {
                        protocol: "HTTP".to_string(),
                        backend_ref: BackendRef {
                            namespace: "default".to_string(),
                            name: "auth".to_string(),
                            port: auth_port,
                            ..BackendRef::default()
                        },
                        http: ntgw_ir::ExternalHTTPAuthConfig {
                            path: "/auth".to_string(),
                            allowed_headers: allowed_headers
                                .into_iter()
                                .map(str::to_string)
                                .collect(),
                            allowed_response_headers: allowed_response_headers
                                .into_iter()
                                .map(str::to_string)
                                .collect(),
                        },
                        grpc: ntgw_ir::ExternalGRPCAuthConfig::default(),
                        forward_body_max_size: None,
                    }),
                    ..Filter::default()
                }],
                backend_refs: vec![BackendRef {
                    namespace: "default".to_string(),
                    name: "backend".to_string(),
                    port: backend_port,
                    ..BackendRef::default()
                }],
                ..HttpRule::default()
            }],
            labels: BTreeMap::new(),
            annotations: BTreeMap::new(),
        }],
        backends: vec![
            BackendCluster {
                ai_service: None,
                token_policy: None,
                name: format!("backend:{backend_port}").into(),
                namespace: "default".to_string().into(),
                protocol: "HTTP".to_string().into(),
                endpoints: vec![BackendEndpoint {
                    address: "127.0.0.1".to_string(),
                    port: backend_port,
                    healthy: true,
                }],
                wasm_plugin: None,
            
                circuit_breaker: None,},
            BackendCluster {
                ai_service: None,
                token_policy: None,
                name: format!("auth:{auth_port}").into(),
                namespace: "default".to_string().into(),
                protocol: "HTTP".to_string().into(),
                endpoints: vec![BackendEndpoint {
                    address: "127.0.0.1".to_string(),
                    port: auth_port,
                    healthy: true,
                }],
                wasm_plugin: None,
            
                circuit_breaker: None,},
        ],
        ..Snapshot::default()
    }));
    let mut s = (**shared.load()).clone();
    s.rebuild_runtime_indexes();
    shared.store(Arc::new(s));
    shared
}

fn external_auth_grpc_snapshot(
    listener_port: u16,
    backend_port: u32,
    auth_port: u32,
    allowed_headers: Vec<&str>,
) -> ntgw_ir::SharedSnapshot {
    let shared = Snapshot::shared();
    shared.store(Arc::new(Snapshot {
        listeners: vec![Listener {
            name: "default/gw/http".to_string(),
            address: "127.0.0.1".to_string(),
            addresses: vec!["127.0.0.1".to_string()],
            port: listener_port as u32,
            protocol: "LISTENER_PROTOCOL_HTTP".to_string(),
            attached_routes: vec!["default/route".to_string()],
            ..Listener::default()
        }],
        http_routes: vec![HttpRoute {
            name: "route".to_string(),
            namespace: "default".to_string(),
            hostnames: Vec::new(),
            parent_refs: vec![ParentRef {
                namespace: "default".to_string(),
                name: "gw".to_string(),
                section_name: String::new(),
                port: listener_port as u32,
                ..ParentRef::default()
            }],
            rules: vec![HttpRule {
                name: String::new(),
                matches: vec![HttpMatch {
                    path: "/protected".to_string(),
                    path_type: "Exact".to_string(),
                    ..HttpMatch::default()
                }],
                filters: vec![Filter {
                    filter_type: "ExternalAuth".to_string(),
                    external_auth: Some(ntgw_ir::ExternalAuthFilter {
                        protocol: "GRPC".to_string(),
                        backend_ref: BackendRef {
                            namespace: "default".to_string(),
                            name: "auth".to_string(),
                            port: auth_port,
                            ..BackendRef::default()
                        },
                        grpc: ntgw_ir::ExternalGRPCAuthConfig {
                            allowed_headers: allowed_headers
                                .into_iter()
                                .map(str::to_string)
                                .collect(),
                        },
                        ..ntgw_ir::ExternalAuthFilter::default()
                    }),
                    ..Filter::default()
                }],
                backend_refs: vec![BackendRef {
                    namespace: "default".to_string(),
                    name: "app".to_string(),
                    port: backend_port,
                    ..BackendRef::default()
                }],
                ..HttpRule::default()
            }],
            ..HttpRoute::default()
        }],
        backends: vec![
            BackendCluster {
                ai_service: None,
                token_policy: None,
                name: format!("app:{backend_port}").into(),
                namespace: "default".to_string().into(),
                protocol: "HTTP".to_string().into(),
                endpoints: vec![BackendEndpoint {
                    address: "127.0.0.1".to_string(),
                    port: backend_port,
                    healthy: true,
                }],
                wasm_plugin: None,
            
                circuit_breaker: None,},
            BackendCluster {
                ai_service: None,
                token_policy: None,
                name: format!("auth:{auth_port}").into(),
                namespace: "default".to_string().into(),
                protocol: "HTTP".to_string().into(),
                endpoints: vec![BackendEndpoint {
                    address: "127.0.0.1".to_string(),
                    port: auth_port,
                    healthy: true,
                }],
                wasm_plugin: None,
            
                circuit_breaker: None,},
        ],
        ..Snapshot::default()
    }));
    let mut s = (**shared.load()).clone();
    s.rebuild_runtime_indexes();
    shared.store(Arc::new(s));
    shared
}

fn enable_external_auth_forward_body(snapshot: &ntgw_ir::SharedSnapshot, max_size: u32) {
    let mut snap = (**snapshot.load()).clone();
    let auth = snap.http_routes[0].rules[0].filters[0]
        .external_auth
        .as_mut()
        .expect("external auth filter");
    auth.forward_body_max_size = Some(max_size);
    snap.rebuild_runtime_indexes();
    snapshot.store(Arc::new(snap));
}

fn grpc_allow_response() -> CheckResponse {
    CheckResponse {
        status: Some(Status {
            code: 0,
            message: String::new(),
            details: Vec::new(),
        }),
        http_response: Some(HttpResponse::OkResponse(OkHttpResponse {
            headers: Vec::new(),
        })),
    }
}

fn grpc_deny_response(status: EnvoyStatusCode, body: &str) -> CheckResponse {
    CheckResponse {
        status: Some(Status {
            code: 7,
            message: "denied".to_string(),
            details: Vec::new(),
        }),
        http_response: Some(HttpResponse::DeniedResponse(DeniedHttpResponse {
            status: Some(HttpStatus {
                code: status as i32,
            }),
            body: body.to_string(),
            headers: Vec::new(),
        })),
    }
}

fn spawn_grpc_auth_server(
    port: u16,
    response: Result<CheckResponse, TonicStatus>,
) -> tokio::sync::mpsc::Receiver<CheckRequest> {
    let (observed_tx, observed_rx) = tokio::sync::mpsc::channel(1);
    let service = TestGrpcAuth {
        response,
        observed: observed_tx,
    };
    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    tokio::spawn(async move {
        TonicServer::builder()
            .add_service(AuthorizationServer::new(service))
            .serve(addr)
            .await
            .expect("grpc auth server");
    });
    observed_rx
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn external_auth_grpc_ok_allows_request_and_sends_selected_headers() {
    let _grpc_auth_lock = GRPC_AUTH_TEST_LOCK.lock().await;
    install_rustls_provider();
    let backend_listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("backend bind");
    let backend_port = backend_listener.local_addr().expect("backend addr").port();
    let auth_port = free_tcp_port();
    let gateway_port = free_tcp_port();
    let snapshot = external_auth_grpc_snapshot(
        gateway_port,
        backend_port as u32,
        auth_port as u32,
        vec!["authorization"],
    );
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

    let mut observed_auth = spawn_grpc_auth_server(auth_port, Ok(grpc_allow_response()));
    let backend = tokio::spawn(async move {
        let (mut stream, _) = timeout(Duration::from_secs(2), backend_listener.accept())
            .await
            .context("backend accept timeout")??;
        let request = read_http_headers(&mut stream).await?;
        assert!(request.starts_with("GET /protected?x=1 HTTP/1.1\r\n"));
        stream
            .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nok")
            .await?;
        Ok::<(), anyhow::Error>(())
    });

    wait_for_listener(gateway_port).await;
    let result = async {
        let mut client = TcpStream::connect(("127.0.0.1", gateway_port)).await?;
        client
            .write_all(
                b"GET /protected?x=1 HTTP/1.1\r\nHost: example.com\r\nAuthorization: Bearer ok\r\nX-Extra: should-not-forward\r\nConnection: close\r\n\r\n",
            )
            .await?;
        let response = read_http_response(&mut client).await?;
        assert!(response.starts_with("HTTP/1.1 200"), "{response}");
        assert!(response.ends_with("\r\n\r\nok"));
        Ok::<(), anyhow::Error>(())
    }
    .await;

    stop_server(server);
    result.expect("client flow");
    let check = timeout(Duration::from_secs(2), observed_auth.recv())
        .await
        .expect("auth request timeout")
        .expect("auth request");
    let http = check
        .attributes
        .expect("attributes")
        .request
        .expect("request")
        .http
        .expect("http request");
    assert_eq!(http.method, "GET");
    assert_eq!(http.path, "/protected?x=1");
    assert_eq!(http.host, "example.com");
    assert_eq!(
        http.headers.get("authorization").map(String::as_str),
        Some("Bearer ok")
    );
    assert!(!http.headers.contains_key("x-extra"));
    assert!(http.body.is_empty());
    backend
        .await
        .expect("backend task")
        .expect("backend result");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn external_auth_grpc_forward_body_sends_buffered_body_to_auth_and_backend() {
    let _grpc_auth_lock = GRPC_AUTH_TEST_LOCK.lock().await;
    install_rustls_provider();
    let backend_listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("backend bind");
    let backend_port = backend_listener.local_addr().expect("backend addr").port();
    let auth_port = free_tcp_port();
    let gateway_port = free_tcp_port();
    let snapshot = external_auth_grpc_snapshot(
        gateway_port,
        backend_port as u32,
        auth_port as u32,
        vec!["authorization"],
    );
    enable_external_auth_forward_body(&snapshot, 32);
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

    let mut observed_auth = spawn_grpc_auth_server(auth_port, Ok(grpc_allow_response()));
    let backend = tokio::spawn(async move {
        let (mut stream, _) = timeout(Duration::from_secs(2), backend_listener.accept())
            .await
            .context("backend accept timeout")??;
        let headers = read_http_headers(&mut stream).await?;
        assert!(headers.starts_with("POST /protected HTTP/1.1\r\n"));
        let body = read_http_body(&mut stream, &headers).await?;
        assert_eq!(body, b"grpc-body".to_vec());
        stream
            .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nok")
            .await?;
        Ok::<(), anyhow::Error>(())
    });

    wait_for_listener(gateway_port).await;
    let result = async {
        let mut client = TcpStream::connect(("127.0.0.1", gateway_port)).await?;
        client
            .write_all(
                b"POST /protected HTTP/1.1\r\nHost: example.com\r\nAuthorization: Bearer ok\r\nContent-Length: 9\r\nConnection: close\r\n\r\ngrpc-body",
            )
            .await?;
        let response = read_http_response(&mut client).await?;
        assert!(response.starts_with("HTTP/1.1 200"), "{response}");
        assert!(response.ends_with("\r\n\r\nok"));
        Ok::<(), anyhow::Error>(())
    }
    .await;

    stop_server(server);
    result.expect("client flow");
    let check = timeout(Duration::from_secs(2), observed_auth.recv())
        .await
        .expect("auth request timeout")
        .expect("auth request");
    let body = check
        .attributes
        .and_then(|attributes| attributes.request)
        .and_then(|request| request.http)
        .map(|http| http.body)
        .expect("auth http body");
    assert_eq!(body, b"grpc-body".to_vec());
    backend
        .await
        .expect("backend task")
        .expect("backend result");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn external_auth_http_forward_body_sends_buffered_body_to_auth_and_backend() {
    install_rustls_provider();
    let backend_listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("backend bind");
    let backend_port = backend_listener.local_addr().expect("backend addr").port();
    let auth_listener = TcpListener::bind("127.0.0.1:0").await.expect("auth bind");
    let auth_port = auth_listener.local_addr().expect("auth addr").port();
    let gateway_port = free_tcp_port();
    let snapshot = external_auth_http_snapshot(
        gateway_port,
        backend_port as u32,
        auth_port as u32,
        vec!["authorization"],
        Vec::new(),
    );
    enable_external_auth_forward_body(&snapshot, 32);
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

    let auth = tokio::spawn(async move {
        let (mut stream, _) = timeout(Duration::from_secs(2), auth_listener.accept())
            .await
            .context("auth accept timeout")??;
        let headers = read_http_headers(&mut stream).await?;
        assert!(headers.starts_with("POST /auth HTTP/1.1\r\n"));
        assert!(
            headers
                .to_ascii_lowercase()
                .contains("content-length: 9\r\n")
        );
        let body = read_http_body(&mut stream, &headers).await?;
        assert_eq!(body, b"auth-body".to_vec());
        stream
            .write_all(b"HTTP/1.1 204 No Content\r\nContent-Length: 0\r\n\r\n")
            .await?;
        Ok::<(), anyhow::Error>(())
    });
    let backend = tokio::spawn(async move {
        let (mut stream, _) = timeout(Duration::from_secs(2), backend_listener.accept())
            .await
            .context("backend accept timeout")??;
        let headers = read_http_headers(&mut stream).await?;
        assert!(headers.starts_with("POST /protected HTTP/1.1\r\n"));
        let body = read_http_body(&mut stream, &headers).await?;
        assert_eq!(body, b"auth-body".to_vec());
        stream
            .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nok")
            .await?;
        Ok::<(), anyhow::Error>(())
    });

    wait_for_listener(gateway_port).await;
    let result = async {
        let mut client = TcpStream::connect(("127.0.0.1", gateway_port)).await?;
        client
            .write_all(
                b"POST /protected HTTP/1.1\r\nHost: example.com\r\nAuthorization: Bearer ok\r\nContent-Length: 9\r\nConnection: close\r\n\r\nauth-body",
            )
            .await?;
        let response = read_http_response(&mut client).await?;
        assert!(response.starts_with("HTTP/1.1 200"), "{response}");
        assert!(response.ends_with("\r\n\r\nok"));
        Ok::<(), anyhow::Error>(())
    }
    .await;

    stop_server(server);
    result.expect("client flow");
    auth.await.expect("auth task").expect("auth result");
    backend
        .await
        .expect("backend task")
        .expect("backend result");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn external_auth_http_forward_body_rejects_oversized_body_before_auth_or_backend() {
    install_rustls_provider();
    let backend_listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("backend bind");
    let backend_port = backend_listener.local_addr().expect("backend addr").port();
    let auth_listener = TcpListener::bind("127.0.0.1:0").await.expect("auth bind");
    let auth_port = auth_listener.local_addr().expect("auth addr").port();
    let gateway_port = free_tcp_port();
    let snapshot = external_auth_http_snapshot(
        gateway_port,
        backend_port as u32,
        auth_port as u32,
        Vec::new(),
        Vec::new(),
    );
    enable_external_auth_forward_body(&snapshot, 4);
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
    let result = async {
        let mut client = TcpStream::connect(("127.0.0.1", gateway_port)).await?;
        client
            .write_all(
                b"POST /protected HTTP/1.1\r\nHost: example.com\r\nContent-Length: 5\r\nConnection: close\r\n\r\nabcde",
            )
            .await?;
        let response = read_http_response(&mut client).await?;
        assert!(response.starts_with("HTTP/1.1 413"), "{response}");
        Ok::<(), anyhow::Error>(())
    }
    .await;

    result.expect("client flow");
    assert!(
        timeout(Duration::from_millis(100), auth_listener.accept())
            .await
            .is_err()
    );
    assert!(
        timeout(Duration::from_millis(100), backend_listener.accept())
            .await
            .is_err()
    );
    stop_server(server);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn external_auth_grpc_denied_response_returns_status_and_body_without_backend() {
    let _grpc_auth_lock = GRPC_AUTH_TEST_LOCK.lock().await;
    install_rustls_provider();
    let backend_listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("backend bind");
    let backend_port = backend_listener.local_addr().expect("backend addr").port();
    let auth_port = free_tcp_port();
    let gateway_port = free_tcp_port();
    let snapshot = external_auth_grpc_snapshot(
        gateway_port,
        backend_port as u32,
        auth_port as u32,
        Vec::new(),
    );
    let runtime = RuntimeOptions {
        enable_ipv6: false,
        ..RuntimeOptions::default()
    };
    let plan = build_listener_plan(&snapshot.load(), &runtime, None).expect("plan");
    let log_path = temp_log_path("grpc-external-auth-denied-access-log");
    let server = start_server(
        plan,
        snapshot,
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

    let mut observed_auth = spawn_grpc_auth_server(
        auth_port,
        Ok(grpc_deny_response(
            EnvoyStatusCode::Forbidden,
            "grpc denied",
        )),
    );
    let backend = tokio::spawn(async move {
        timeout(Duration::from_millis(250), backend_listener.accept())
            .await
            .expect_err("denied request should not reach backend");
        Ok::<(), anyhow::Error>(())
    });

    wait_for_listener(gateway_port).await;
    let result = async {
        let mut client = TcpStream::connect(("127.0.0.1", gateway_port)).await?;
        client
            .write_all(b"GET /protected HTTP/1.1\r\nHost: example.com\r\nX-Extra: forwarded-when-list-empty\r\nConnection: close\r\n\r\n")
            .await?;
        let response = read_http_response(&mut client).await?;
        assert!(response.starts_with("HTTP/1.1 403"));
        assert!(response.ends_with("\r\n\r\ngrpc denied"));
        Ok::<(), anyhow::Error>(())
    }
    .await;

    stop_server(server);
    result.expect("client flow");
    let check = timeout(Duration::from_secs(2), observed_auth.recv())
        .await
        .expect("auth request timeout")
        .expect("auth request");
    let http = check
        .attributes
        .expect("attributes")
        .request
        .expect("request")
        .http
        .expect("http request");
    assert_eq!(
        http.headers.get("x-extra").map(String::as_str),
        Some("forwarded-when-list-empty")
    );
    backend
        .await
        .expect("backend task")
        .expect("backend result");

    let log_contents = wait_for_log_contents(&log_path).await;
    assert!(log_contents.contains("11 -"));

    shutdown_access_log_writer(&log_path.display().to_string());
    let _ = fs::remove_file(log_path);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn external_auth_grpc_rpc_error_fails_closed_without_backend() {
    let _grpc_auth_lock = GRPC_AUTH_TEST_LOCK.lock().await;
    install_rustls_provider();
    let backend_listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("backend bind");
    let backend_port = backend_listener.local_addr().expect("backend addr").port();
    let auth_port = free_tcp_port();
    let gateway_port = free_tcp_port();
    let snapshot = external_auth_grpc_snapshot(
        gateway_port,
        backend_port as u32,
        auth_port as u32,
        Vec::new(),
    );
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

    let _observed_auth =
        spawn_grpc_auth_server(auth_port, Err(TonicStatus::unavailable("auth unavailable")));
    let backend = tokio::spawn(async move {
        timeout(Duration::from_millis(250), backend_listener.accept())
            .await
            .expect_err("auth failure should not reach backend");
        Ok::<(), anyhow::Error>(())
    });

    wait_for_listener(gateway_port).await;
    let result = async {
        let mut client = TcpStream::connect(("127.0.0.1", gateway_port)).await?;
        client
            .write_all(b"GET /protected HTTP/1.1\r\nHost: example.com\r\nConnection: close\r\n\r\n")
            .await?;
        let response = read_http_response(&mut client).await?;
        assert!(response.starts_with("HTTP/1.1 500"));
        Ok::<(), anyhow::Error>(())
    }
    .await;

    stop_server(server);
    result.expect("client flow");
    backend
        .await
        .expect("backend task")
        .expect("backend result");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn external_auth_http_always_forwards_authorization_even_without_allowed_headers() {
    install_rustls_provider();
    let backend_listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("backend bind");
    let backend_port = backend_listener.local_addr().expect("backend addr").port();
    let auth_listener = TcpListener::bind("127.0.0.1:0").await.expect("auth bind");
    let auth_port = auth_listener.local_addr().expect("auth addr").port();
    let gateway_port = free_tcp_port();
    let snapshot = external_auth_http_snapshot(
        gateway_port,
        backend_port as u32,
        auth_port as u32,
        Vec::new(),
        Vec::new(),
    );
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

    let auth = tokio::spawn(async move {
        let (mut stream, _) = timeout(Duration::from_secs(2), auth_listener.accept())
            .await
            .context("auth accept timeout")??;
        let request = read_http_headers(&mut stream).await?;
        assert!(
            request
                .to_ascii_lowercase()
                .contains("authorization: bearer ok\r\n")
        );
        assert!(
            !request
                .to_ascii_lowercase()
                .contains("x-extra: should-not-forward\r\n")
        );
        stream
            .write_all(b"HTTP/1.1 204 No Content\r\nContent-Length: 0\r\n\r\n")
            .await?;
        Ok::<(), anyhow::Error>(())
    });
    let backend = tokio::spawn(async move {
        let (mut stream, _) = timeout(Duration::from_secs(2), backend_listener.accept())
            .await
            .context("backend accept timeout")??;
        let request = read_http_headers(&mut stream).await?;
        assert!(request.starts_with("GET /protected HTTP/1.1\r\n"));
        stream
            .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nok")
            .await?;
        Ok::<(), anyhow::Error>(())
    });

    wait_for_listener(gateway_port).await;
    let result = async {
        let mut client = TcpStream::connect(("127.0.0.1", gateway_port)).await?;
        client
            .write_all(
                b"GET /protected HTTP/1.1\r\nHost: example.com\r\nAuthorization: Bearer ok\r\nX-Extra: should-not-forward\r\nConnection: close\r\n\r\n",
            )
            .await?;
        let response = read_http_response(&mut client).await?;
        assert!(response.starts_with("HTTP/1.1 200"));
        assert!(response.ends_with("\r\n\r\nok"));
        Ok::<(), anyhow::Error>(())
    }
    .await;

    stop_server(server);
    result.expect("client flow");
    auth.await.expect("auth task").expect("auth result");
    backend
        .await
        .expect("backend task")
        .expect("backend result");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn external_auth_http_copies_allowed_auth_response_headers_to_backend_request() {
    install_rustls_provider();
    let backend_listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("backend bind");
    let backend_port = backend_listener.local_addr().expect("backend addr").port();
    let auth_listener = TcpListener::bind("127.0.0.1:0").await.expect("auth bind");
    let auth_port = auth_listener.local_addr().expect("auth addr").port();
    let gateway_port = free_tcp_port();
    let snapshot = external_auth_http_snapshot(
        gateway_port,
        backend_port as u32,
        auth_port as u32,
        vec!["authorization"],
        vec!["x-user", "x-scope", "host"],
    );
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

    let auth = tokio::spawn(async move {
        let (mut stream, _) = timeout(Duration::from_secs(2), auth_listener.accept())
            .await
            .context("auth accept timeout")??;
        let request = read_http_headers(&mut stream).await?;
        assert!(
            request
                .to_ascii_lowercase()
                .contains("authorization: bearer ok\r\n")
        );
        stream
            .write_all(
                b"HTTP/1.1 204 No Content\r\nContent-Length: 0\r\nX-User: alice\r\nX-Scope: admin\r\nHost: auth.example\r\n\r\n",
            )
            .await?;
        Ok::<(), anyhow::Error>(())
    });
    let backend = tokio::spawn(async move {
        let (mut stream, _) = timeout(Duration::from_secs(2), backend_listener.accept())
            .await
            .context("backend accept timeout")??;
        let request = read_http_headers(&mut stream).await?;
        let lower_request = request.to_ascii_lowercase();
        assert!(lower_request.contains("x-user: alice\r\n"));
        assert!(lower_request.contains("x-scope: admin\r\n"));
        assert!(!lower_request.contains("host: auth.example\r\n"));
        stream
            .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nok")
            .await?;
        Ok::<(), anyhow::Error>(())
    });

    wait_for_listener(gateway_port).await;
    let result = async {
        let mut client = TcpStream::connect(("127.0.0.1", gateway_port)).await?;
        client
            .write_all(
                b"GET /protected HTTP/1.1\r\nHost: example.com\r\nAuthorization: Bearer ok\r\nConnection: close\r\n\r\n",
            )
            .await?;
        let response = read_http_response(&mut client).await?;
        assert!(response.starts_with("HTTP/1.1 200"));
        assert!(response.ends_with("\r\n\r\nok"));
        Ok::<(), anyhow::Error>(())
    }
    .await;

    stop_server(server);
    result.expect("client flow");
    auth.await.expect("auth task").expect("auth result");
    backend
        .await
        .expect("backend task")
        .expect("backend result");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn external_auth_http_2xx_allows_request_to_backend() {
    install_rustls_provider();
    let backend_listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("backend bind");
    let backend_port = backend_listener.local_addr().expect("backend addr").port();
    let auth_listener = TcpListener::bind("127.0.0.1:0").await.expect("auth bind");
    let auth_port = auth_listener.local_addr().expect("auth addr").port();
    let gateway_port = free_tcp_port();
    let snapshot = external_auth_http_snapshot(
        gateway_port,
        backend_port as u32,
        auth_port as u32,
        vec!["authorization"],
        Vec::new(),
    );
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

    let auth = tokio::spawn(async move {
        let (mut stream, _) = timeout(Duration::from_secs(2), auth_listener.accept())
            .await
            .context("auth accept timeout")??;
        let request = read_http_headers(&mut stream).await?;
        assert!(request.starts_with("GET /auth HTTP/1.1\r\n"));
        assert!(
            request
                .to_ascii_lowercase()
                .contains("authorization: bearer ok\r\n")
        );
        stream
            .write_all(b"HTTP/1.1 204 No Content\r\nContent-Length: 0\r\n\r\n")
            .await?;
        Ok::<(), anyhow::Error>(())
    });
    let backend = tokio::spawn(async move {
        let (mut stream, _) = timeout(Duration::from_secs(2), backend_listener.accept())
            .await
            .context("backend accept timeout")??;
        let request = read_http_headers(&mut stream).await?;
        assert!(request.starts_with("GET /protected HTTP/1.1\r\n"));
        stream
            .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nok")
            .await?;
        Ok::<(), anyhow::Error>(())
    });

    wait_for_listener(gateway_port).await;
    let result = async {
        let mut client = TcpStream::connect(("127.0.0.1", gateway_port)).await?;
        client
            .write_all(
                b"GET /protected HTTP/1.1\r\nHost: example.com\r\nAuthorization: Bearer ok\r\nConnection: close\r\n\r\n",
            )
            .await?;
        let response = read_http_response(&mut client).await?;
        assert!(response.starts_with("HTTP/1.1 200"));
        assert!(response.ends_with("\r\n\r\nok"));
        Ok::<(), anyhow::Error>(())
    }
    .await;

    stop_server(server);
    result.expect("client flow");
    auth.await.expect("auth task").expect("auth result");
    backend
        .await
        .expect("backend task")
        .expect("backend result");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn external_auth_http_non_2xx_denies_without_backend() {
    install_rustls_provider();
    let backend_listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("backend bind");
    let backend_port = backend_listener.local_addr().expect("backend addr").port();
    let auth_listener = TcpListener::bind("127.0.0.1:0").await.expect("auth bind");
    let auth_port = auth_listener.local_addr().expect("auth addr").port();
    let gateway_port = free_tcp_port();
    let snapshot = external_auth_http_snapshot(
        gateway_port,
        backend_port as u32,
        auth_port as u32,
        vec!["authorization"],
        Vec::new(),
    );
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

    let auth = tokio::spawn(async move {
        let (mut stream, _) = timeout(Duration::from_secs(2), auth_listener.accept())
            .await
            .context("auth accept timeout")??;
        let request = read_http_headers(&mut stream).await?;
        assert!(request.starts_with("GET /auth HTTP/1.1\r\n"));
        stream
            .write_all(b"HTTP/1.1 403 Forbidden\r\nContent-Length: 6\r\n\r\ndeny!!")
            .await?;
        Ok::<(), anyhow::Error>(())
    });
    let backend = tokio::spawn(async move {
        timeout(Duration::from_millis(250), backend_listener.accept())
            .await
            .expect_err("denied request should not reach backend");
        Ok::<(), anyhow::Error>(())
    });

    wait_for_listener(gateway_port).await;
    let result = async {
        let mut client = TcpStream::connect(("127.0.0.1", gateway_port)).await?;
        client
            .write_all(b"GET /protected HTTP/1.1\r\nHost: example.com\r\nConnection: close\r\n\r\n")
            .await?;
        let response = read_http_response(&mut client).await?;
        assert!(response.starts_with("HTTP/1.1 403"));
        assert!(response.ends_with("\r\n\r\ndeny!!"));
        Ok::<(), anyhow::Error>(())
    }
    .await;

    stop_server(server);
    result.expect("client flow");
    auth.await.expect("auth task").expect("auth result");
    backend
        .await
        .expect("backend task")
        .expect("backend result");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn external_auth_http_connection_error_fails_closed_without_backend() {
    install_rustls_provider();
    let backend_listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("backend bind");
    let backend_port = backend_listener.local_addr().expect("backend addr").port();
    let auth_port = free_tcp_port();
    let gateway_port = free_tcp_port();
    let snapshot = external_auth_http_snapshot(
        gateway_port,
        backend_port as u32,
        auth_port as u32,
        vec!["authorization"],
        Vec::new(),
    );
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

    let backend = tokio::spawn(async move {
        timeout(Duration::from_millis(250), backend_listener.accept())
            .await
            .expect_err("auth failure should not reach backend");
        Ok::<(), anyhow::Error>(())
    });

    wait_for_listener(gateway_port).await;
    let result = async {
        let mut client = TcpStream::connect(("127.0.0.1", gateway_port)).await?;
        client
            .write_all(b"GET /protected HTTP/1.1\r\nHost: example.com\r\nConnection: close\r\n\r\n")
            .await?;
        let response = read_http_response(&mut client).await?;
        assert!(response.starts_with("HTTP/1.1 500"));
        Ok::<(), anyhow::Error>(())
    }
    .await;

    stop_server(server);
    result.expect("client flow");
    backend
        .await
        .expect("backend task")
        .expect("backend result");
}

#[test]
fn external_auth_backend_tls_validation_in_snapshot() {
    use ntgw_ir::{BackendTlsValidation, Snapshot};

    let auth_port: u32 = 8443;
    let snapshot = Snapshot {
        backends: vec![ntgw_ir::BackendCluster {
            ai_service: None,
            token_policy: None,
            name: format!("auth:{auth_port}").into(),
            namespace: "default".to_string().into(),
            protocol: "HTTPS".to_string().into(),
            endpoints: vec![ntgw_ir::BackendEndpoint {
                address: "127.0.0.1".to_string(),
                port: auth_port,
                healthy: true,
            }],
            wasm_plugin: None,
        
                circuit_breaker: None,}],
        backend_policies: {
            let mut m = std::collections::BTreeMap::new();
            m.insert(
                format!("default/auth:{auth_port}"),
                ntgw_ir::BackendPolicy {
                    tls_validation: Some(BackendTlsValidation {
                        hostname: "auth.default.svc.cluster.local".to_string(),
                        use_system_ca_certificates: true,
                        ca_pems: vec![],
                        subject_alt_names: vec![],
                        min_version: String::new(),
                        max_version: String::new(),
                    }),
                    ..ntgw_ir::BackendPolicy::default()
                },
            );
            m
        },
        ..Snapshot::default()
    };

    let policy = snapshot
        .backend_policy(&format!("default/auth:{auth_port}"))
        .expect("auth backend policy");

    let validation = policy
        .tls_validation
        .as_ref()
        .expect("auth backend tls validation");
    assert_eq!(validation.hostname, "auth.default.svc.cluster.local");
    assert!(validation.use_system_ca_certificates);
    assert!(validation.ca_pems.is_empty());
}

#[test]
fn external_auth_with_backend_tls_and_session_persistence_combo_snapshot() {
    use ntgw_ir::{
        BackendPolicy, BackendTlsValidation, LoadBalancingPolicy, SessionPersistence, Snapshot,
    };

    let snapshot = Snapshot {
        backends: vec![
            ntgw_ir::BackendCluster {
                ai_service: None,
                token_policy: None,
                name: "backend:8080".to_string().into(),
                namespace: "default".to_string().into(),
                protocol: "HTTP".to_string().into(),
                endpoints: vec![ntgw_ir::BackendEndpoint {
                    address: "10.0.0.1".to_string(),
                    port: 8080,
                    healthy: true,
                }],
                wasm_plugin: None,
            
                circuit_breaker: None,},
            ntgw_ir::BackendCluster {
                ai_service: None,
                token_policy: None,
                name: "auth:8443".to_string().into(),
                namespace: "default".to_string().into(),
                protocol: "HTTPS".to_string().into(),
                endpoints: vec![ntgw_ir::BackendEndpoint {
                    address: "127.0.0.1".to_string(),
                    port: 8443,
                    healthy: true,
                }],
                wasm_plugin: None,
            
                circuit_breaker: None,},
        ],
        backend_policies: {
            let mut m = std::collections::BTreeMap::new();
            // Backend policy: TLS validation + session persistence + load balancing
            m.insert(
                "default/backend:8080".to_string(),
                BackendPolicy {
                    session_persistence: Some(SessionPersistence {
                        session_name: "test-session".to_string(),
                        session_type: "Cookie".to_string(),
                        absolute_timeout: None,
                        idle_timeout: None,
                        cookie: Some(ntgw_ir::CookieConfig {
                            lifetime_type: "Session".to_string(),
                        }),
                    }),
                    load_balancing: Some(LoadBalancingPolicy {
                        policy_type: "ConsistentHash".to_string(),
                        consistent_hash: Some(ntgw_ir::ConsistentHashPolicy {
                            key_type: "SourceIP".to_string(),
                            header_name: String::new(),
                        }),
                        slow_start: None,
                    }),
                    ..BackendPolicy::default()
                },
            );
            // Auth backend: TLS validation only
            m.insert(
                "default/auth:8443".to_string(),
                BackendPolicy {
                    tls_validation: Some(BackendTlsValidation {
                        hostname: "auth.default.svc.cluster.local".to_string(),
                        use_system_ca_certificates: true,
                        ca_pems: vec![],
                        subject_alt_names: vec![],
                        min_version: String::new(),
                        max_version: String::new(),
                    }),
                    ..BackendPolicy::default()
                },
            );
            m
        },
        ..Snapshot::default()
    };

    // Verify backend policy has session persistence + load balancing + no TLS
    let backend_policy = snapshot
        .backend_policy("default/backend:8080")
        .expect("backend policy");
    assert!(backend_policy.session_persistence.is_some());
    assert_eq!(
        backend_policy
            .session_persistence
            .as_ref()
            .unwrap()
            .session_name,
        "test-session"
    );
    assert!(backend_policy.load_balancing.is_some());
    assert_eq!(
        backend_policy.load_balancing.as_ref().unwrap().policy_type,
        "ConsistentHash"
    );
    assert!(backend_policy.tls_validation.is_none());

    // Verify auth backend policy has TLS validation only
    let auth_policy = snapshot
        .backend_policy("default/auth:8443")
        .expect("auth backend policy");
    assert!(auth_policy.tls_validation.is_some());
    assert!(auth_policy.session_persistence.is_none());
    assert!(auth_policy.load_balancing.is_none());
}
