#[tokio::test]
async fn proxies_udp_datagrams() -> Result<()> {
    let upstream = UdpSocket::bind("127.0.0.1:0").await?;
    let upstream_addr = upstream.local_addr()?;
    let downstream = Arc::new(UdpSocket::bind("127.0.0.1:0").await?);
    let client = UdpSocket::bind("127.0.0.1:0").await?;
    let client_addr = client.local_addr()?;
    let listener = test_listener("default/gw/udp", downstream.local_addr()?.port() as u32);
    let snapshot = test_snapshot(listener.clone(), upstream_addr);

    let upstream_task = tokio::spawn(async move {
        let mut buf = [0; 64];
        let (size, peer) = upstream.recv_from(&mut buf).await?;
        assert_eq!(&buf[..size], b"ping");
        upstream.send_to(b"pong", peer).await?;
        Ok::<(), anyhow::Error>(())
    });

    let proxy_task = tokio::spawn(proxy_datagram(
        snapshot,
        listener.name,
        Arc::clone(&downstream),
        client_addr,
        b"ping".to_vec(),
        disabled_access_log(),
        SharedTrafficStats::shared(),
        Duration::from_millis(500),
    ));

    let mut buf = [0; 64];
    let (size, from) = client.recv_from(&mut buf).await?;
    assert_eq!(&buf[..size], b"pong");
    assert_eq!(from, downstream.local_addr()?);

    upstream_task.await??;
    proxy_task.await??;
    Ok(())
}

#[tokio::test]
async fn proxies_udp_datagrams_emit_runtime_ids_in_access_log() -> Result<()> {
    let upstream = UdpSocket::bind("127.0.0.1:0").await?;
    let upstream_addr = upstream.local_addr()?;
    let downstream = Arc::new(UdpSocket::bind("127.0.0.1:0").await?);
    let client = UdpSocket::bind("127.0.0.1:0").await?;
    let client_addr = client.local_addr()?;
    let listener = test_listener("default/gw/udp", downstream.local_addr()?.port() as u32);
    let snapshot = test_snapshot(listener.clone(), upstream_addr);
    rebuild_runtime_indexes(&snapshot);
    let expected_runtime_ids = selected_runtime_ids(&snapshot, &listener.name);
    let log_path = temp_log_path("udp-runtime-ids");

    let upstream_task = tokio::spawn(async move {
        let mut buf = [0; 64];
        let (size, peer) = upstream.recv_from(&mut buf).await?;
        assert_eq!(&buf[..size], b"ping");
        upstream.send_to(b"pong", peer).await?;
        Ok::<(), anyhow::Error>(())
    });

    let proxy_task = tokio::spawn(proxy_datagram(
        snapshot,
        listener.name,
        Arc::clone(&downstream),
        client_addr,
        b"ping".to_vec(),
        json_access_log(&log_path),
        SharedTrafficStats::shared(),
        Duration::from_millis(10),
    ));

    let mut buf = [0; 64];
    let (size, from) = client.recv_from(&mut buf).await?;
    assert_eq!(&buf[..size], b"pong");
    assert_eq!(from, downstream.local_addr()?);

    upstream_task.await??;
    proxy_task.await??;

    let contents = wait_for_log_contents(&log_path, "\"event\":\"udp_datagram\"").await?;
    assert_runtime_id_fields(&contents, expected_runtime_ids);
    cleanup_access_log(&log_path);
    Ok(())
}

#[tokio::test]
async fn forwards_multiple_udp_responses_before_idle_timeout() -> Result<()> {
    let upstream = UdpSocket::bind("127.0.0.1:0").await?;
    let upstream_addr = upstream.local_addr()?;
    let downstream = Arc::new(UdpSocket::bind("127.0.0.1:0").await?);
    let client = UdpSocket::bind("127.0.0.1:0").await?;
    let client_addr = client.local_addr()?;
    let listener = test_listener("default/gw/udp", downstream.local_addr()?.port() as u32);
    let snapshot = test_snapshot(listener.clone(), upstream_addr);

    let upstream_task = tokio::spawn(async move {
        let mut buf = [0; 64];
        let (size, peer) = upstream.recv_from(&mut buf).await?;
        assert_eq!(&buf[..size], b"dns");
        upstream.send_to(b"first", peer).await?;
        upstream.send_to(b"second", peer).await?;
        Ok::<(), anyhow::Error>(())
    });

    let proxy_task = tokio::spawn(proxy_datagram(
        snapshot,
        listener.name,
        Arc::clone(&downstream),
        client_addr,
        b"dns".to_vec(),
        disabled_access_log(),
        SharedTrafficStats::shared(),
        Duration::from_millis(500),
    ));

    let mut first = [0; 64];
    let mut second = [0; 64];
    let (first_size, _) = client.recv_from(&mut first).await?;
    let (second_size, _) = client.recv_from(&mut second).await?;
    assert_eq!(&first[..first_size], b"first");
    assert_eq!(&second[..second_size], b"second");

    upstream_task.await??;
    proxy_task.await??;
    Ok(())
}

#[tokio::test]
async fn proxy_datagram_uses_configured_idle_timeout() -> Result<()> {
    let upstream = UdpSocket::bind("127.0.0.1:0").await?;
    let upstream_addr = upstream.local_addr()?;
    let downstream = Arc::new(UdpSocket::bind("127.0.0.1:0").await?);
    let client = UdpSocket::bind("127.0.0.1:0").await?;
    let client_addr = client.local_addr()?;
    let listener = test_listener("default/gw/udp", downstream.local_addr()?.port() as u32);
    let snapshot = test_snapshot(listener.clone(), upstream_addr);

    let upstream_task = tokio::spawn(async move {
        let mut buf = [0; 64];
        let (_size, _peer) = upstream.recv_from(&mut buf).await?;
        tokio::time::sleep(Duration::from_millis(40)).await;
        Ok::<(), anyhow::Error>(())
    });

    let started = std::time::Instant::now();
    proxy_datagram(
        snapshot,
        listener.name,
        Arc::clone(&downstream),
        client_addr,
        b"dns".to_vec(),
        disabled_access_log(),
        SharedTrafficStats::shared(),
        Duration::from_millis(10),
    )
    .await?;

    assert!(
        started.elapsed() < Duration::from_millis(30),
        "configured idle timeout should be honored"
    );

    upstream_task.await??;
    let _ = client;
    Ok(())
}
