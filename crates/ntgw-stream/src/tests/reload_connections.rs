use super::*;
use std::net::SocketAddr;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
    time::{sleep, timeout},
};

#[tokio::test]
async fn tcp_route_reload_preserves_existing_connection() -> Result<()> {
    let gateway_port = free_tcp_port().await?;
    let gateway_addr = format!("127.0.0.1:{gateway_port}");
    let listener = tcp_listener(gateway_port);
    let upstream_a = TcpListener::bind("127.0.0.1:0").await?;
    let upstream_a_addr = upstream_a.local_addr()?;
    let upstream_b = TcpListener::bind("127.0.0.1:0").await?;
    let upstream_b_addr = upstream_b.local_addr()?;
    let snapshot = ntgw_ir::Snapshot::shared();
    *snapshot.write() = tcp_snapshot("v1", listener.clone(), "upstream-a", upstream_a_addr);
    let updates = ntgw_ir::SnapshotSignal::shared();
    let runtime_stats = RuntimeStats::shared();
    let traffic = SharedTrafficStats::shared();
    let overload = ntgw_observability::OverloadStats::shared();
    let (shutdown_tx, shutdown_rx) = watch::channel(false);
    let (config_tx, config_rx) = watch::channel(Arc::new(reloadable_config(16 * 1024)));
    let runtime_task = tokio::spawn(run(
        snapshot.clone(),
        updates.clone(),
        config_rx,
        runtime_stats.clone(),
        traffic,
        ntgw_observability::UdpSessionStats::shared(),
        overload,
        shutdown_rx,
    ));

    wait_for_stream_listener_attempt(
        runtime_stats.clone(),
        "default/gw/tcp",
        "v1",
        1,
        Duration::from_secs(1),
    )
    .await?;

    let upstream_a_task = tokio::spawn(async move {
        let (mut stream, _) = upstream_a.accept().await?;
        expect_message(&mut stream, b"ping", b"a-one").await?;
        expect_message(&mut stream, b"stay", b"a-two").await?;
        Ok::<(), anyhow::Error>(())
    });

    let mut first = TcpStream::connect(&gateway_addr).await?;
    first.write_all(b"ping").await?;
    assert_response(&mut first, b"a-one").await?;

    let previous = snapshot.read().clone();
    let mut next = tcp_snapshot("v2", listener, "upstream-b", upstream_b_addr);
    next.inherit_runtime_state_from(&previous);
    *snapshot.write() = next;
    updates.notify_changed();
    config_tx
        .send(Arc::new(reloadable_config(8 * 1024)))
        .expect("runtime config receiver should be alive");

    wait_for_stream_listener_attempt(
        runtime_stats.clone(),
        "default/gw/tcp",
        "v2",
        2,
        Duration::from_secs(1),
    )
    .await?;

    first.write_all(b"stay").await?;
    assert_response(&mut first, b"a-two").await?;

    let upstream_b_task = tokio::spawn(async move {
        let (mut stream, _) = upstream_b.accept().await?;
        expect_message(&mut stream, b"ping", b"b-one").await?;
        Ok::<(), anyhow::Error>(())
    });

    let mut second = TcpStream::connect(&gateway_addr).await?;
    second.write_all(b"ping").await?;
    second.shutdown().await?;
    assert_response(&mut second, b"b-one").await?;
    first.shutdown().await?;

    upstream_a_task.await??;
    upstream_b_task.await??;
    shutdown_tx.send(true).expect("shutdown runtime");
    runtime_task.await??;
    Ok(())
}

fn reloadable_config(tcp_proxy_buffer_bytes: usize) -> ReloadableRuntimeConfig {
    ReloadableRuntimeConfig {
        runtime: RuntimeOptions {
            reload_retry_interval: Duration::from_millis(20),
            tcp_proxy_buffer_bytes,
            ..RuntimeOptions::default()
        },
        access_log: AccessLogOptions {
            enabled: false,
            ..AccessLogOptions::default()
        },
    }
}

fn tcp_listener(port: u16) -> Listener {
    Listener {
        name: "default/gw/tcp".to_string(),
        address: "127.0.0.1".to_string(),
        addresses: vec!["127.0.0.1".to_string()],
        port: u32::from(port),
        protocol: "LISTENER_PROTOCOL_TCP".to_string(),
        attached_routes: vec!["default/tcp-route".to_string()],
        ..Listener::default()
    }
}

fn tcp_snapshot(
    version: &str,
    listener: Listener,
    backend_name: &str,
    upstream_addr: SocketAddr,
) -> ntgw_ir::Snapshot {
    let mut snapshot = ntgw_ir::Snapshot {
        id: version.to_string(),
        listeners: vec![listener.clone()],
        stream_routes: vec![ntgw_ir::StreamRoute {
            name: "tcp-route".to_string(),
            namespace: "default".to_string(),
            kind: "ROUTE_KIND_TCP".to_string(),
            parent_refs: Vec::new(),
            rules: vec![ntgw_ir::StreamRule {
                name: String::new(),
                matches: vec![ntgw_ir::StreamMatch {
                    port: listener.port,
                    sni_hostname: String::new(),
                    mode: ntgw_ir::TlsRouteMode::default(),
                }],
                backend_refs: vec![ntgw_ir::BackendRef {
                    namespace: "default".to_string(),
                    name: backend_name.to_string(),
                    port: upstream_addr.port() as u32,
                    ..ntgw_ir::BackendRef::default()
                }],
            }],
            labels: std::collections::BTreeMap::new(),
            annotations: std::collections::BTreeMap::new(),
        }],
        backends: vec![ntgw_ir::BackendCluster {
            ai_service: None,
            token_policy: None,
            name: format!("{backend_name}:{}", upstream_addr.port()),
            namespace: "default".to_string(),
            protocol: "TCP".to_string(),
            endpoints: vec![ntgw_ir::BackendEndpoint {
                address: upstream_addr.ip().to_string(),
                port: upstream_addr.port() as u32,
                healthy: true,
            }],
            wasm_plugin: None,
        }],
        ..ntgw_ir::Snapshot::default()
    };
    snapshot.rebuild_runtime_indexes();
    snapshot
}

async fn free_tcp_port() -> Result<u16> {
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    Ok(listener.local_addr()?.port())
}

async fn wait_for_stream_listener_attempt(
    runtime_stats: ntgw_observability::SharedRuntimeStats,
    listener: &str,
    version: &str,
    attempts: u64,
    max_wait: Duration,
) -> Result<()> {
    timeout(max_wait, async {
        loop {
            let snapshot = runtime_stats.snapshot();
            let progress = snapshot.stream_listener_progress.get(listener);
            if snapshot.stream_last_good_reload_version == version
                && progress.is_some_and(|progress| {
                    progress.last_good_version == version && progress.attempts >= attempts
                })
            {
                return Ok(());
            }
            sleep(Duration::from_millis(10)).await;
        }
    })
    .await?
}

async fn expect_message(stream: &mut TcpStream, expected: &[u8], response: &[u8]) -> Result<()> {
    let mut buf = vec![0; expected.len()];
    stream.read_exact(&mut buf).await?;
    assert_eq!(buf, expected);
    stream.write_all(response).await?;
    Ok(())
}

async fn assert_response(stream: &mut TcpStream, expected: &[u8]) -> Result<()> {
    let mut buf = vec![0; expected.len()];
    stream.read_exact(&mut buf).await?;
    assert_eq!(buf, expected);
    Ok(())
}
