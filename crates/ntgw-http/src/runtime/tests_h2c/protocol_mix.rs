    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn h2c_backend_uses_prior_knowledge_preface() {
        install_rustls_provider();
        let upstream_listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("upstream bind");
        let upstream_addr = upstream_listener.local_addr().expect("upstream addr");
        let gateway_port = free_tcp_port();
        let snapshot =
            simple_http_snapshot(gateway_port, "/h2c", upstream_addr.port() as u32, "H2C");
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
            let (mut stream, _) = upstream_listener.accept().await?;
            let mut preface = [0; 24];
            stream.read_exact(&mut preface).await?;
            assert_eq!(&preface, b"PRI * HTTP/2.0\r\n\r\nSM\r\n\r\n");
            Ok::<(), anyhow::Error>(())
        });

        wait_for_listener(gateway_port).await;

        let _ = async {
            let mut client = TcpStream::connect(("127.0.0.1", gateway_port)).await?;
            client
                .write_all(b"GET /h2c HTTP/1.1\r\nHost: example.com\r\n\r\n")
                .await?;
            let _ = read_all_with_timeout(&mut client).await;
            Ok::<(), anyhow::Error>(())
        }
        .await;

        stop_server(server);
        upstream
            .await
            .expect("upstream task")
            .expect("upstream result");
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn http1_and_h2c_backends_coexist_on_same_listener() {
        install_rustls_provider();
        let http_listener = TcpListener::bind("127.0.0.1:0").await.expect("http bind");
        let http_addr = http_listener.local_addr().expect("http addr");
        let h2c_listener = TcpListener::bind("127.0.0.1:0").await.expect("h2c bind");
        let h2c_addr = h2c_listener.local_addr().expect("h2c addr");
        let gateway_port = free_tcp_port();
        let snapshot = dual_protocol_snapshot(
            gateway_port,
            http_addr.port() as u32,
            h2c_addr.port() as u32,
        );
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

        let http_upstream = tokio::spawn(async move {
            let (mut stream, _) = http_listener.accept().await?;
            let request = read_http_headers(&mut stream).await?;
            assert!(request.starts_with("GET /http HTTP/1.1\r\n"));
            stream
                .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nok")
                .await?;
            Ok::<(), anyhow::Error>(())
        });
        let h2c_upstream = tokio::spawn(async move {
            let (mut stream, _) = h2c_listener.accept().await?;
            let mut preface = [0; 24];
            stream.read_exact(&mut preface).await?;
            assert_eq!(&preface, b"PRI * HTTP/2.0\r\n\r\nSM\r\n\r\n");
            Ok::<(), anyhow::Error>(())
        });

        wait_for_listener(gateway_port).await;

        let result = async {
            let mut http_client = TcpStream::connect(("127.0.0.1", gateway_port)).await?;
            http_client
                .write_all(b"GET /http HTTP/1.1\r\nHost: example.com\r\n\r\n")
                .await?;
            let response = read_all_with_timeout(&mut http_client).await?;
            let response = String::from_utf8(response).expect("http response utf8");
            assert!(response.starts_with("HTTP/1.1 200"));

            let mut h2c_client = TcpStream::connect(("127.0.0.1", gateway_port)).await?;
            h2c_client
                .write_all(b"GET /h2c HTTP/1.1\r\nHost: example.com\r\n\r\n")
                .await?;
            let _ = read_all_with_timeout(&mut h2c_client).await;
            Ok::<(), anyhow::Error>(())
        }
        .await;

        stop_server(server);
        result.expect("coexist client flow");
        http_upstream
            .await
            .expect("http upstream task")
            .expect("http upstream result");
        h2c_upstream
            .await
            .expect("h2c upstream task")
            .expect("h2c upstream result");
    }
