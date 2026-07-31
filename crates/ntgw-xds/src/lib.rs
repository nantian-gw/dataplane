#![forbid(unsafe_code)]

use std::sync::Arc;
use std::time::Duration;
use std::time::Instant;

use anyhow::Result;
use tokio::{
    sync::{mpsc, watch},
    time::sleep,
};
use tokio_stream::wrappers::ReceiverStream;
use tonic::transport::{Channel, Endpoint};
use tracing::{debug, info, warn};

use opentelemetry::propagation::{Extractor, TextMapPropagator};
use opentelemetry_sdk::propagation::TraceContextPropagator;

use crate::features::{preflight_required_features, supported_features};
use ntgw_ir::{SharedSnapshot, SharedSnapshotSignal, Snapshot};
use ntgw_observability::SharedRuntimeStats;
use ntgw_proto::gateway::control::v1::{
    ConfigSnapshot, configuration_discovery_service_client::ConfigurationDiscoveryServiceClient,
    delta_discovery_service_client::DeltaDiscoveryServiceClient,
};

pub mod bench;
pub mod delta;
mod features;
mod reconnect;
mod stats;
mod status;
mod stream;
mod tls;

#[cfg(test)]
mod bench_tests;

#[cfg(test)]
mod tests;

#[cfg(test)]
#[test]
fn discovery_messages_include_supported_features() {
    tests::run_exact_discovery_messages_include_supported_features();
}

#[cfg(test)]
#[test]
fn preflight_required_features_reports_sorted_missing_features() {
    tests::run_exact_preflight_required_features_reports_sorted_missing_features();
}

#[cfg(test)]
#[test]
fn preflight_required_features_accepts_supported_snapshot() {
    tests::run_exact_preflight_required_features_accepts_supported_snapshot();
}

#[cfg(test)]
#[test]
fn preflight_rejection_keeps_last_good_snapshot() {
    tests::run_exact_preflight_rejection_keeps_last_good_snapshot();
}

pub use reconnect::ReconnectBackoff;
pub(crate) use reconnect::{
    log_duplicate_snapshot_skipped, log_heartbeat_report_failure, log_stream_failure_retry,
    retry_delay_after_stream_failure,
};
pub use stats::{ClientStats, ClientStatsSnapshot, SharedClientStats};
pub(crate) use status::{
    RuntimeApplyRequirements, build_status_report, discovery_ack, discovery_nack, discovery_open,
    snapshot_runtime_apply_requirements, wait_for_runtime_apply_result,
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
    ClientTlsOptions, ConnectOptions, TransportOptions, build_client_tls_config, normalize_endpoint,
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

/// Minimal [Extractor] adapter that presents a single traceparent header value
/// to the W3C [TraceContextPropagator] for cross-component trace linking.
struct TraceparentCarrier<'a>(&'a str);

impl<'a> Extractor for TraceparentCarrier<'a> {
    fn get(&self, key: &str) -> Option<&str> {
        if key.eq_ignore_ascii_case("traceparent") {
            Some(self.0)
        } else {
            None
        }
    }

    fn keys(&self) -> Vec<&str> {
        vec!["traceparent"]
    }
}

pub async fn connect_delta_channel(
    options: ConnectOptions,
) -> Result<DeltaDiscoveryServiceClient<Channel>> {
    tls::ensure_rustls_provider();
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
    Ok(DeltaDiscoveryServiceClient::new(endpoint.connect().await?))
}

pub struct ControlPlaneClient {
    inner: ConfigurationDiscoveryServiceClient<Channel>,
    transport: TransportOptions,
}

impl ControlPlaneClient {
    pub async fn connect(options: ConnectOptions) -> Result<Self> {
        tls::ensure_rustls_provider();
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
        let version = { snapshot.load().id.clone() };
        let supported_features = supported_features();
        if let Err(err) = tx
            .send(discovery_open(
                &node_id,
                &cluster,
                &version,
                &supported_features,
            ))
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
                    if !config.traceparent.is_empty() {
                        let propagator = TraceContextPropagator::new();
                        let _parent_cx = propagator.extract(&TraceparentCarrier(&config.traceparent));
                        debug!(
                            traceparent = %config.traceparent,
                            "received snapshot with trace context"
                        );
                    }
                    let next_version =
                        snapshot_version_from_response(message.version.as_str(), &config);
                    let version = if should_apply_snapshot(
                        snapshot.load().id.as_str(),
                        next_version.as_deref(),
                    ) {
                        if let Err((version, error_detail)) = preflight_snapshot_before_swap(
                            &config,
                            next_version.as_deref(),
                            &supported_features,
                            &stats,
                        ) {
                            warn!(
                                version = %version,
                                compatibility_profile = %config.compatibility_profile,
                                error = %error_detail,
                                "rejected snapshot before decode due to unsupported required features"
                            );
                            tx.send(discovery_nack(
                                &node_id,
                                &cluster,
                                &version,
                                &message.nonce,
                                &error_detail,
                                &supported_features,
                            ))
                            .await?;
                            continue;
                        }
                        let stage = Instant::now();
                        let mut next = Snapshot::from_proto_without_runtime_indexes(config);
                        observe_apply_stage_elapsed(&stats, "decode", stage);
                        let version = next_version.unwrap_or_else(|| next.id.clone());
                        {
                            let stage = Instant::now();
                            let current = snapshot.load();
                            next.inherit_runtime_state_values_from(&current);
                            observe_apply_stage_elapsed(&stats, "inherit_runtime_state", stage);
                        }
                        let stage = Instant::now();
                        next.rebuild_runtime_indexes();
                        observe_apply_stage_elapsed(&stats, "rebuild_indexes", stage);
                        let stage = Instant::now();
                        snapshot.store(Arc::new(next));
                        updates.notify_changed();
                        observe_apply_stage_elapsed(&stats, "snapshot_swap", stage);
                        let apply_requirements = {
                            let current = snapshot.load();
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
                                    &supported_features,
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
                                    &supported_features,
                                ))
                                .await?;
                                observe_apply_stage_elapsed(&stats, "ack_wait", stage);
                            }
                        }
                        version
                    } else {
                        let version = next_version.unwrap_or_else(|| snapshot.load().id.clone());
                        stats.observe_snapshot_skipped();
                        log_duplicate_snapshot_skipped(&version);
                        tx.send(discovery_ack(
                            &node_id,
                            &cluster,
                            &version,
                            &message.nonce,
                            &supported_features,
                        ))
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

pub(crate) fn preflight_snapshot_before_swap(
    config: &ConfigSnapshot,
    version_hint: Option<&str>,
    supported_features: &[String],
    stats: &SharedClientStats,
) -> std::result::Result<(), (String, String)> {
    match preflight_required_features(config, supported_features) {
        Ok(()) => Ok(()),
        Err(error_detail) => {
            let version = version_hint
                .map(str::trim)
                .filter(|version| !version.is_empty())
                .map(ToOwned::to_owned)
                .unwrap_or_else(|| config.id.clone());
            stats.observe_snapshot_nacked(&version, &error_detail);
            Err((version, error_detail))
        }
    }
}
