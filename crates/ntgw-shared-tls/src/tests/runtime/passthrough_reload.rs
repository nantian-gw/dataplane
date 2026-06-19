use super::*;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

#[tokio::test]
async fn tls_passthrough_reload_preserves_existing_connection() -> Result<()> {
    let stream_backend_a = TcpListener::bind("127.0.0.1:0").await?;
    let stream_backend_a_addr = stream_backend_a.local_addr()?;
    let stream_backend_b = TcpListener::bind("127.0.0.1:0").await?;
    let stream_backend_b_addr = stream_backend_b.local_addr()?;
    let gateway_port = free_tcp_port();
    let bind_addr = format!("127.0.0.1:{gateway_port}");
    let http_backend_port = free_tcp_port();
    let snapshot = shared_tls_snapshot(
        gateway_port,
        http_backend_port,
        stream_backend_a_addr.port(),
    );
    let mut s = (**snapshot.load()).clone();
    s.id = "v1".to_string();
    snapshot.store(Arc::new(s));
    let plan = build_listener_plan(&snapshot.load(), &RuntimeOptions::default())?;
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
        ntgw_observability::OverloadStats::shared(),
        HttpCircuitBreakerController::new(Default::default()),
        HttpRateLimitController::new(Default::default()),
        RetryBudgetController::new(Default::default()),
        None,
    )?;
    let (_shutdown_tx, shutdown_rx) = watch::channel(false);
    let hello = build_client_hello("passthrough.example.com");

    let upstream_a = tokio::spawn(expect_passthrough_backend(
        stream_backend_a,
        hello.len(),
        [
            (b"ping".as_slice(), b"a-one".as_slice()),
            (b"stay", b"a-two"),
        ],
    ));
    let server_snapshot = snapshot.clone();
    let server = tokio::spawn(async move {
        let mut tasks = Vec::new();
        for _ in 0..2 {
            let (stream, _) = gateway_listener.accept().await?;
            let bind = bind.clone();
            let snapshot = server_snapshot.clone();
            let app = app.clone();
            let shutdown_rx = shutdown_rx.clone();
            tasks.push(tokio::spawn(async move {
                crate::runtime::handle_connection(
                    bind,
                    stream,
                    snapshot,
                    app,
                    shutdown_rx,
                    ConnectionConfig,
                )
                .await
            }));
        }
        for task in tasks {
            task.await??;
        }
        Ok::<(), anyhow::Error>(())
    });

    let mut first = TcpStream::connect(&bind_addr).await?;
    first.write_all(&hello).await?;
    first.write_all(b"ping").await?;
    assert_passthrough_response(&mut first, b"a-one").await?;

    let previous = (**snapshot.load()).clone();
    let mut next = (**shared_tls_snapshot(
        gateway_port,
        http_backend_port,
        stream_backend_b_addr.port(),
    )
    .load())
    .clone();
    next.id = "v2".to_string();
    next.inherit_runtime_state_from(&previous);
    snapshot.store(Arc::new(next));

    let upstream_b = tokio::spawn(expect_passthrough_backend(
        stream_backend_b,
        hello.len(),
        [(b"ping".as_slice(), b"b-one".as_slice())],
    ));

    let mut second = TcpStream::connect(&bind_addr).await?;
    second.write_all(&hello).await?;
    second.write_all(b"ping").await?;
    second.shutdown().await?;
    assert_passthrough_response(&mut second, b"b-one").await?;

    first.write_all(b"stay").await?;
    assert_passthrough_response(&mut first, b"a-two").await?;
    first.shutdown().await?;

    upstream_a.await??;
    upstream_b.await??;
    server.await??;
    Ok(())
}

async fn expect_passthrough_backend<const N: usize>(
    listener: TcpListener,
    client_hello_len: usize,
    exchanges: [(&[u8], &[u8]); N],
) -> Result<()> {
    let (mut stream, _) = listener.accept().await?;
    let mut hello = vec![0; client_hello_len];
    stream.read_exact(&mut hello).await?;
    assert!(hello.starts_with(&[0x16, 0x03]));

    for (expected, response) in exchanges {
        let mut buf = vec![0; expected.len()];
        stream.read_exact(&mut buf).await?;
        assert_eq!(buf, expected);
        stream.write_all(response).await?;
    }

    Ok(())
}

async fn assert_passthrough_response(stream: &mut TcpStream, expected: &[u8]) -> Result<()> {
    let mut buf = vec![0; expected.len()];
    stream.read_exact(&mut buf).await?;
    assert_eq!(buf, expected);
    Ok(())
}
