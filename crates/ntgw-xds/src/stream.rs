use std::{future::Future, time::Duration};

use anyhow::{anyhow, Result};
use tokio::{task::JoinHandle, time::MissedTickBehavior};
use tonic::transport::Channel;

use ntgw_ir::SharedSnapshot;
use ntgw_observability::SharedRuntimeStats;
use ntgw_proto::gateway::control::v1::{
    configuration_discovery_service_client::ConfigurationDiscoveryServiceClient, ConfigSnapshot,
};

use crate::{build_status_report, log_heartbeat_report_failure, HEARTBEAT_INTERVAL};

pub(crate) fn snapshot_version_from_response(
    message_version: &str,
    snapshot: &ConfigSnapshot,
) -> Option<String> {
    let payload_version = snapshot.id.trim();
    if !payload_version.is_empty() {
        return Some(payload_version.to_string());
    }

    let message_version = message_version.trim();
    (!message_version.is_empty()).then(|| message_version.to_string())
}

pub(crate) fn should_apply_snapshot(current_version: &str, next_version: Option<&str>) -> bool {
    next_version.is_none_or(|version| current_version != version)
}

pub(super) fn spawn_status_heartbeat(
    mut client: ConfigurationDiscoveryServiceClient<Channel>,
    node_id: String,
    snapshot: SharedSnapshot,
    runtime: SharedRuntimeStats,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(HEARTBEAT_INTERVAL);
        interval.set_missed_tick_behavior(MissedTickBehavior::Skip);

        loop {
            interval.tick().await;
            if let Err(err) = client
                .report_status(build_status_report(&node_id, &snapshot, &runtime, false))
                .await
            {
                log_heartbeat_report_failure(&node_id, &err);
            }
        }
    })
}

pub(crate) async fn wait_for_stream_message<F, T>(
    future: F,
    stale_stream_timeout: Duration,
) -> Result<Option<T>>
where
    F: Future<Output = std::result::Result<Option<T>, tonic::Status>>,
{
    match tokio::time::timeout(stale_stream_timeout, future).await {
        Ok(result) => Ok(result?),
        Err(_) => Err(anyhow!(
            "stale xds stream: no control-plane message received for {stale_stream_timeout:?}"
        )),
    }
}
