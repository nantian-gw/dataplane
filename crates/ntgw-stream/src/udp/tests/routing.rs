#[tokio::test]
async fn returns_error_when_no_udp_route_matches() -> Result<()> {
    let downstream = Arc::new(UdpSocket::bind("127.0.0.1:0").await?);
    let client = UdpSocket::bind("127.0.0.1:0").await?;
    let listener = test_listener("default/gw/udp", downstream.local_addr()?.port() as u32);
    let snapshot = Snapshot::shared();
    snapshot.store(Arc::new(Snapshot {
        listeners: vec![listener.clone()],
        ..Snapshot::default()
    }));
    let err = proxy_datagram(
        snapshot,
        listener.name,
        downstream,
        client.local_addr()?,
        b"ping".to_vec(),
        disabled_access_log(),
        SharedTrafficStats::shared(),
        Duration::from_millis(500),
    )
    .await
    .unwrap_err();

    assert_eq!(
        err.to_string(),
        "stream dispatch error: no stream route matched listener default/gw/udp"
    );
    Ok(())
}
