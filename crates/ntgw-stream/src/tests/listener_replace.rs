use super::*;

#[tokio::test]
async fn replace_reports_listener_start_failure() -> Result<()> {
    let occupied = TcpListener::bind("127.0.0.1:0").await?;
    let bind = occupied.local_addr()?.to_string();
    let mut listeners = ListenerSet::default();
    let pool = std::sync::Arc::new(crate::pool::TcpConnectionPool::new(
        0,
        tokio::time::Duration::from_secs(30),
    ));

    let result = listeners
        .replace(
            Some(ListenerPlan {
                listeners: vec![PlannedListener {
                    name: "default/gw/tcp".to_string().into(),
                    bind,
                    protocol: StreamProtocol::Tcp,
                }],
            }),
            ntgw_ir::Snapshot::shared(),
            AccessLogOptions {
                enabled: false,
                ..AccessLogOptions::default()
            },
            SharedTrafficStats::shared(),
            ntgw_observability::UdpSessionStats::shared(),
            TcpAdmissionController::new(
                TcpAdmissionOptions::default(),
                ntgw_observability::OverloadStats::shared(),
            ),
            UdpAdmissionController::new(
                UdpAdmissionOptions::default(),
                ntgw_observability::OverloadStats::shared(),
            ),
            tokio::time::Duration::from_millis(500),
            16 * 1024,
            None,
            None,
            pool,
            false,
        )
        .await;

    assert!(
        result
            .first_error
            .as_deref()
            .is_some_and(|message| !message.is_empty())
    );
    assert!(listeners.tasks.is_empty());
    Ok(())
}
