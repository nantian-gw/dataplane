#[tokio::test]
async fn udp_listener_datagram_budget_drops_second_datagram() -> Result<()> {
    let upstream = UdpSocket::bind("127.0.0.1:0").await?;
    let upstream_addr = upstream.local_addr()?;
    let downstream = Arc::new(UdpSocket::bind("127.0.0.1:0").await?);
    let gateway_addr = downstream.local_addr()?;
    let client = UdpSocket::bind("127.0.0.1:0").await?;
    let listener = test_listener("default/gw/udp", gateway_addr.port() as u32);
    let snapshot = test_snapshot(listener.clone(), upstream_addr);
    let overload = aeg_observability::OverloadStats::shared();
    let (shutdown_tx, shutdown_rx) = watch::channel(false);
    let server = tokio::spawn(run_with_socket(
        snapshot,
        listener.name.clone(),
        gateway_addr.to_string(),
        Arc::clone(&downstream),
        shutdown_rx,
        disabled_access_log(),
        SharedTrafficStats::shared(),
        aeg_observability::UdpAdmissionController::new(
            aeg_observability::UdpAdmissionOptions {
                listener_datagram_limit: 1,
                ..aeg_observability::UdpAdmissionOptions::default()
            },
            overload.clone(),
        ),
        UdpSessionStats::shared(),
        Duration::from_millis(500),
    ));

    let (accepted_tx, accepted_rx) = tokio::sync::oneshot::channel();
    let (release_tx, release_rx) = tokio::sync::oneshot::channel();
    let upstream_task = tokio::spawn(async move {
        let mut buf = [0; 64];
        let (size, peer) = upstream.recv_from(&mut buf).await?;
        assert_eq!(&buf[..size], b"first");
        let _ = accepted_tx.send(());
        let _ = release_rx.await;
        upstream.send_to(b"pong", peer).await?;
        Ok::<(), anyhow::Error>(())
    });

    client.send_to(b"first", gateway_addr).await?;
    accepted_rx
        .await
        .expect("first datagram should reach upstream");

    client.send_to(b"second", gateway_addr).await?;
    let mut second_buf = [0; 64];
    let second_result = timeout(
        Duration::from_millis(200),
        client.recv_from(&mut second_buf),
    )
    .await;
    assert!(
        second_result.is_err(),
        "second datagram should be dropped under overload"
    );

    release_tx
        .send(())
        .expect("release first datagram response");
    let mut response = [0; 64];
    let (size, _) = client.recv_from(&mut response).await?;
    assert_eq!(&response[..size], b"pong");

    shutdown_tx.send(true).expect("shutdown udp listener");
    upstream_task.await??;
    server.await??;

    let overload_snapshot = overload.snapshot();
    assert_eq!(overload_snapshot.udp_rejected_total, 1);
    assert_eq!(overload_snapshot.udp_rejected_listener_total, 1);
    Ok(())
}
