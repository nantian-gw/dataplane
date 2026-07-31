use anyhow::Result;
use std::sync::Arc;
use tokio::net::{TcpListener, TcpStream};

use super::*;

#[tokio::test]
async fn returns_error_when_no_stream_route_matches() -> Result<()> {
    let gateway_listener = TcpListener::bind("127.0.0.1:0").await?;
    let gateway_addr = gateway_listener.local_addr()?;
    let listener = test_listener(
        "default/gw/tcp",
        gateway_addr.port() as u32,
        "LISTENER_PROTOCOL_TCP",
    );
    let snapshot = Snapshot::shared();
    snapshot.store(Arc::new(Snapshot {
        listeners: vec![listener.clone()],
        ..Snapshot::default()
    }));

    let server = tokio::spawn(async move {
        let (stream, _) = gateway_listener.accept().await?;
        handle_connection(
            snapshot,
            listener.name,
            stream,
            false,
            disabled_access_log(),
            SharedTrafficStats::shared(),
            TCP_PROXY_BUFFER_BYTES,
            None,
            None,
            std::sync::Arc::new(crate::pool::TcpConnectionPool::new(
                0,
                Duration::from_secs(30),
            )),
        )
        .await
    });

    let client = TcpStream::connect(gateway_addr).await?;
    drop(client);

    let err = server
        .await
        .expect("server task should join")
        .expect_err("handle_connection should fail without a matching route");
    assert_eq!(
        err.to_string(),
        "stream dispatch error: no stream route matched listener default/gw/tcp"
    );
    Ok(())
}

#[tokio::test]
async fn records_upstream_failure_when_backend_connect_fails() -> Result<()> {
    let unavailable_backend = TcpListener::bind("127.0.0.1:0").await?;
    let upstream_addr = unavailable_backend.local_addr()?;
    drop(unavailable_backend);

    let gateway_listener = TcpListener::bind("127.0.0.1:0").await?;
    let gateway_addr = gateway_listener.local_addr()?;
    let listener = test_listener(
        "default/gw/tcp",
        gateway_addr.port() as u32,
        "LISTENER_PROTOCOL_TCP",
    );
    let snapshot = test_snapshot(
        listener.clone(),
        "tcp-route",
        "ROUTE_KIND_TCP",
        vec![ntgw_ir::StreamMatch {
            port: gateway_addr.port() as u32,
            sni_hostname: String::new(),
            mode: ntgw_ir::TlsRouteMode::default(),
        }],
        upstream_addr,
    );
    let traffic = SharedTrafficStats::shared();
    let server_traffic = traffic.clone();

    let server = tokio::spawn(async move {
        let (stream, _) = gateway_listener.accept().await?;
        handle_connection(
            snapshot,
            listener.name,
            stream,
            false,
            disabled_access_log(),
            server_traffic,
            TCP_PROXY_BUFFER_BYTES,
            None,
            None,
            std::sync::Arc::new(crate::pool::TcpConnectionPool::new(
                0,
                Duration::from_secs(30),
            )),
        )
        .await
    });

    let client = TcpStream::connect(gateway_addr).await?;
    drop(client);

    server
        .await
        .expect("server task should join")
        .expect_err("handle_connection should fail when backend connect fails");

    let stats = traffic.snapshot();
    assert_eq!(stats.total_events, 1);
    assert_eq!(stats.total_request_events, 0);
    assert!(
        stats.response_flags.is_empty(),
        "TCP upstream failures must not pollute request response flag counters: {:?}",
        stats.response_flags
    );
    assert_eq!(stats.total_upstream_pool_hits, 0);
    assert_eq!(stats.total_upstream_pool_misses, 0);
    assert_eq!(stats.total_upstream_connect_latency_observations, 0);
    assert_eq!(stats.total_bytes_received, 0);
    assert_eq!(stats.total_bytes_sent, 0);
    Ok(())
}
