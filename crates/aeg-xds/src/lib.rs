#![forbid(unsafe_code)]

use std::time::Duration;
use std::time::Instant;

use anyhow::Result;
use tokio::{
    sync::{mpsc, watch},
    time::sleep,
};
use tokio_stream::wrappers::ReceiverStream;
use tonic::transport::{Channel, Endpoint};
use tracing::{info, warn};

use aeg_ir::{SharedSnapshot, SharedSnapshotSignal, Snapshot};
use aeg_observability::SharedRuntimeStats;
use aeg_proto::gateway::control::v1::{
    configuration_discovery_service_client::ConfigurationDiscoveryServiceClient, DiscoveryRequest,
    DiscoveryResultStatus,
};

pub mod bench;
mod reconnect;
mod stats;
mod status;
mod stream;
mod tls;

#[cfg(test)]
mod bench_tests;

#[cfg(test)]
mod tests;

pub use reconnect::ReconnectBackoff;
pub(crate) use reconnect::{
    log_duplicate_snapshot_skipped, log_heartbeat_report_failure, log_stream_failure_retry,
    retry_delay_after_stream_failure,
};
pub use stats::{ClientStats, ClientStatsSnapshot, SharedClientStats};
pub(crate) use status::{
    build_status_report, discovery_ack, discovery_nack, snapshot_runtime_apply_requirements,
    wait_for_runtime_apply_result, RuntimeApplyRequirements,
};
#[cfg(test)]
pub(crate) use status::{
    SNAPSHOT_APPLIED_MESSAGE, SNAPSHOT_REJECTED_MESSAGE_PREFIX, WAITING_FOR_SNAPSHOT_MESSAGE,
};
pub(crate) use stream::{
    should_apply_snapshot, snapshot_version_from_response, spawn_status_heartbeat,
    wait_for_stream_message,
};
pub use tls::{
    build_client_tls_config, normalize_endpoint, ClientTlsOptions, ConnectOptions, TransportOptions,
};

const HEARTBEAT_INTERVAL: Duration = Duration::from_secs(10);

struct StreamRunResult {
    established: bool,
    result: Result<()>,
}

#[derive(Clone)]
pub struct ControlPlaneRunArgs {
    pub node_id: String,
    pub cluster: String,
    pub snapshot: SharedSnapshot,
    pub updates: SharedSnapshotSignal,
    pub runtime: SharedRuntimeStats,
    pub stats: SharedClientStats,
    pub shutdown: watch::Receiver<bool>,
}

pub struct ControlPlaneClient {
    inner: ConfigurationDiscoveryServiceClient<Channel>,
    transport: TransportOptions,
}

impl ControlPlaneClient {
    pub async fn connect(options: ConnectOptions) -> Result<Self> {
        let endpoint = normalize_endpoint(&options.endpoint, options.tls.is_some())?;
        let mut endpoint = Endpoint::from_shared(endpoint)?;
        endpoint = endpoint
            .connect_timeout(options.transport.connect_timeout)
            .http2_keep_alive_interval(options.transport.keepalive_interval)
            .keep_alive_timeout(options.transport.keepalive_timeout)
            .keep_alive_while_idle(true);
        if let Some(tls) = options.tls.as_ref() {
            endpoint = endpoint.tls_config(build_client_tls_config(tls)?)?;
        }
        let inner = ConfigurationDiscoveryServiceClient::new(endpoint.connect().await?);
        Ok(Self {
            inner,
            transport: options.transport,
        })
    }

    pub async fn run(mut self, mut args: ControlPlaneRunArgs) -> Result<()> {
        let mut reconnect_backoff = ReconnectBackoff::new(&self.transport);
        loop {
            if *args.shutdown.borrow() {
                return Ok(());
            }

            let stream_run = self.run_stream(args.clone()).await;
            if *args.shutdown.borrow() {
                return Ok(());
            }
            if let Err(err) = stream_run.result {
                args.stats
                    .observe_stream_failure_with_error(err.to_string().as_str());
                let delay = retry_delay_after_stream_failure(
                    &mut reconnect_backoff,
                    stream_run.established,
                );
                log_stream_failure_retry(&err, delay);
                tokio::select! {
                    _ = sleep(delay) => {}
                    _ = args.shutdown.changed() => return Ok(()),
                }
            }
        }
    }

    async fn run_stream(&mut self, args: ControlPlaneRunArgs) -> StreamRunResult {
        let ControlPlaneRunArgs {
            node_id,
            cluster,
            snapshot,
            updates,
            runtime,
            stats,
            mut shutdown,
        } = args;
        let (tx, rx) = mpsc::channel(8);
        let mut status_client = self.inner.clone();
        let version = { snapshot.read().id.clone() };
        if let Err(err) = tx
            .send(DiscoveryRequest {
                node_id: node_id.clone(),
                cluster: cluster.clone(),
                version,
                nonce: String::new(),
                subscriptions: vec!["*".to_string()],
                result_status: DiscoveryResultStatus::Unspecified as i32,
                error_detail: String::new(),
            })
            .await
        {
            return StreamRunResult {
                established: false,
                result: Err(err.into()),
            };
        }

        let response = match self
            .inner
            .stream_configuration(ReceiverStream::new(rx))
            .await
        {
            Ok(response) => response,
            Err(err) => {
                return StreamRunResult {
                    established: false,
                    result: Err(err.into()),
                };
            }
        };
        let mut stream = response.into_inner();
        stats.observe_stream_connected();
        let heartbeat = spawn_status_heartbeat(
            status_client.clone(),
            node_id.clone(),
            snapshot.clone(),
            runtime.clone(),
        );

        let result = async {
            loop {
                if *shutdown.borrow() {
                    break;
                }

                let next = tokio::select! {
                    result = wait_for_stream_message(
                        stream.message(),
                        self.transport.stale_stream_timeout,
                    ) => result?,
                    _ = shutdown.changed() => break,
                };
                let Some(message) = next else {
                    break;
                };
                stats.observe_control_plane_contact();
                if let Some(config) = message.snapshot {
                    let next_version =
                        snapshot_version_from_response(message.version.as_str(), &config);
                    let version = if should_apply_snapshot(
                        snapshot.read().id.as_str(),
                        next_version.as_deref(),
                    ) {
                        let stage = Instant::now();
                        let mut next = Snapshot::from_proto_without_runtime_indexes(config);
                        observe_apply_stage_elapsed(&stats, "decode", stage);
                        let version = next_version.unwrap_or_else(|| next.id.clone());
                        {
                            let stage = Instant::now();
                            let current = snapshot.read();
                            next.inherit_runtime_state_values_from(&current);
                            observe_apply_stage_elapsed(&stats, "inherit_runtime_state", stage);
                        }
                        let stage = Instant::now();
                        next.rebuild_runtime_indexes();
                        observe_apply_stage_elapsed(&stats, "rebuild_indexes", stage);
                        let stage = Instant::now();
                        *snapshot.write() = next;
                        updates.notify_changed();
                        observe_apply_stage_elapsed(&stats, "snapshot_swap", stage);
                        let apply_requirements = {
                            let current = snapshot.read();
                            snapshot_runtime_apply_requirements(&current)
                        };
                        let stage = Instant::now();
                        match wait_for_runtime_apply_result(
                            runtime.clone(),
                            &version,
                            apply_requirements,
                            &self.transport,
                        )
                        .await
                        {
                            Ok(()) => {
                                observe_apply_stage_elapsed(&stats, "listener_apply", stage);
                                stats.observe_snapshot_applied(&version);
                                info!(version = %version, "applied snapshot");
                                let stage = Instant::now();
                                tx.send(discovery_ack(
                                    &node_id,
                                    &cluster,
                                    &version,
                                    &message.nonce,
                                ))
                                .await?;
                                status_client
                                    .report_status(build_status_report(
                                        &node_id, &snapshot, &runtime, true,
                                    ))
                                    .await?;
                                observe_apply_stage_elapsed(&stats, "ack_wait", stage);
                            }
                            Err(error_detail) => {
                                observe_apply_stage_elapsed(&stats, "listener_apply", stage);
                                stats.observe_snapshot_nacked(&version, &error_detail);
                                warn!(
                                    version = %version,
                                    error = %error_detail,
                                    "rejected snapshot after runtime apply failure"
                                );
                                let stage = Instant::now();
                                tx.send(discovery_nack(
                                    &node_id,
                                    &cluster,
                                    &version,
                                    &message.nonce,
                                    &error_detail,
                                ))
                                .await?;
                                observe_apply_stage_elapsed(&stats, "ack_wait", stage);
                            }
                        }
                        version
                    } else {
                        let version = next_version.unwrap_or_else(|| snapshot.read().id.clone());
                        stats.observe_snapshot_skipped();
                        log_duplicate_snapshot_skipped(&version);
                        tx.send(discovery_ack(&node_id, &cluster, &version, &message.nonce))
                            .await?;
                        status_client
                            .report_status(build_status_report(&node_id, &snapshot, &runtime, true))
                            .await?;
                        version
                    };
                    let _ = version;
                }
            }
            Ok(())
        }
        .await;
        stats.observe_stream_disconnected();
        heartbeat.abort();
        let _ = heartbeat.await;

        StreamRunResult {
            established: true,
            result,
        }
    }
}

fn observe_apply_stage_elapsed(stats: &SharedClientStats, stage: &str, started_at: Instant) {
    stats.observe_apply_stage_duration(
        stage,
        started_at.elapsed().as_millis().min(u64::MAX as u128) as u64,
    );
}
