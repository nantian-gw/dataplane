use super::*;

#[tokio::test]
async fn tcp_and_udp_admission_track_listener_scope_fast_fail() {
    let stats = OverloadStats::shared();
    let tcp = TcpAdmissionController::new(
        TcpAdmissionOptions {
            listener_connection_limit: 1,
            ..TcpAdmissionOptions::default()
        },
        stats.clone(),
    );
    let udp = UdpAdmissionController::new(
        UdpAdmissionOptions {
            listener_datagram_limit: 1,
            ..UdpAdmissionOptions::default()
        },
        stats.clone(),
    );

    let tcp_permit = tcp
        .try_acquire("default/gw/tcp")
        .expect("first tcp connection should succeed");
    let tcp_rejection = tcp
        .try_acquire("default/gw/tcp")
        .expect_err("second tcp connection should fail");
    assert_eq!(tcp_rejection.scope_label(), "listener");

    let udp_permit = udp
        .try_acquire("default/gw/udp")
        .expect("first udp datagram should succeed");
    let udp_rejection = udp
        .try_acquire("default/gw/udp")
        .expect_err("second udp datagram should fail");
    assert_eq!(udp_rejection.scope_label(), "listener");

    let snapshot = stats.snapshot();
    assert_eq!(snapshot.tcp_rejected_total, 1);
    assert_eq!(snapshot.tcp_rejected_listener_total, 1);
    assert_eq!(
        snapshot
            .tcp_rejected_listener_by_name
            .get("default/gw/tcp")
            .copied(),
        Some(1)
    );
    assert_eq!(
        snapshot
            .tcp_listener_connections_current
            .get("default/gw/tcp")
            .copied(),
        Some(1)
    );
    assert_eq!(snapshot.udp_rejected_total, 1);
    assert_eq!(snapshot.udp_rejected_listener_total, 1);
    assert_eq!(
        snapshot
            .udp_rejected_listener_by_name
            .get("default/gw/udp")
            .copied(),
        Some(1)
    );
    assert_eq!(
        snapshot
            .udp_listener_datagrams_current
            .get("default/gw/udp")
            .copied(),
        Some(1)
    );

    drop(tcp_permit);
    drop(udp_permit);

    let snapshot = stats.snapshot();
    assert!(snapshot.tcp_listener_connections_current.is_empty());
    assert!(snapshot.udp_listener_datagrams_current.is_empty());
}
