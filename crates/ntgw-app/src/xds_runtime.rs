use std::sync::Arc;

use tokio::{sync::watch, time::sleep};
use tracing::error;

use ntgw_ir::{SharedSnapshot, SharedSnapshotSignal};
use ntgw_observability::SharedRuntimeStats;
use ntgw_xds::{
    ConnectOptions, ControlPlaneClient, ControlPlaneRunArgs, ReconnectBackoff, SharedClientStats,
};

#[derive(Clone)]
pub(crate) struct XdsRuntimeConfig {
    pub(crate) connect_options: ConnectOptions,
    pub(crate) node_id: String,
    pub(crate) cluster: String,
    pub(crate) xds_protocol: String,
}

pub(crate) async fn run_xds_loop(
    mut config: watch::Receiver<Arc<XdsRuntimeConfig>>,
    snapshot: SharedSnapshot,
    updates: SharedSnapshotSignal,
    runtime: SharedRuntimeStats,
    stats: SharedClientStats,
    mut shutdown: watch::Receiver<bool>,
) {
    let mut current = config.borrow().clone();
    let mut reconnect_backoff = ReconnectBackoff::new(&current.connect_options.transport);

    loop {
        if *shutdown.borrow() {
            break;
        }

        if config.has_changed().unwrap_or(false) {
            current = config.borrow_and_update().clone();
            reconnect_backoff = ReconnectBackoff::new(&current.connect_options.transport);
        }

        let connect_options = current.connect_options.clone();
        let node_id = current.node_id.clone();
        let cluster = current.cluster.clone();
        let connect = ControlPlaneClient::connect(connect_options.clone());
        tokio::pin!(connect);

        match tokio::select! {
            result = &mut connect => result,
            _ = config.changed() => {
                current = config.borrow_and_update().clone();
                reconnect_backoff = ReconnectBackoff::new(&current.connect_options.transport);
                continue;
            }
            _ = shutdown.changed() => break,
        } {
            Ok(client) => {
                reconnect_backoff.reset();
                let (stop_tx, stop_rx) = watch::channel(false);
                if current.xds_protocol == "delta" {
                    match ntgw_xds::connect_delta_channel(connect_options.clone()).await {
                        Ok(delta_client) => {
                            let on_snapshot = {
                                let snapshot = snapshot.clone();
                                let updates = updates.clone();
                                move |config| {
                                    let mut next =
                                        ntgw_ir::Snapshot::from_proto_without_runtime_indexes(
                                            config,
                                        );
                                    {
                                        let current = snapshot.load();
                                        next.inherit_runtime_state_values_from(&current);
                                    }
                                    next.rebuild_runtime_indexes();
                                    snapshot.store(Arc::new(next));
                                    updates.notify_changed();
                                }
                            };
                            let delta_run = ntgw_xds::delta::delta_connect_loop(
                                node_id.clone(),
                                cluster.clone(),
                                stop_rx,
                                delta_client,
                                on_snapshot,
                            );
                            tokio::pin!(delta_run);
                            tokio::select! {
                                result = &mut delta_run => {
                                    if let Err(err) = result {
                                        error!(error = %err, "delta xDS client exited");
                                    }
                                }
                                _ = config.changed() => {
                                    current = config.borrow_and_update().clone();
                                    reconnect_backoff = ReconnectBackoff::new(
                                        &current.connect_options.transport,
                                    );
                                    let _ = stop_tx.send(true);
                                    let _ = (&mut delta_run).await;
                                    continue;
                                }
                                _ = shutdown.changed() => {
                                    let _ = stop_tx.send(true);
                                    let _ = (&mut delta_run).await;
                                    break;
                                }
                            }
                        }
                        Err(err) => {
                            stats.observe_connect_failure_with_error(err.to_string().as_str());
                            error!(error = %err, "failed to connect delta xDS channel");
                        }
                    }
                } else {
                    let run = client.run(ControlPlaneRunArgs {
                        node_id: node_id.clone(),
                        cluster: cluster.clone(),
                        snapshot: snapshot.clone(),
                        updates: updates.clone(),
                        runtime: runtime.clone(),
                        stats: stats.clone(),
                        shutdown: stop_rx,
                    });
                    tokio::pin!(run);
                    tokio::select! {
                        result = &mut run => {
                            if let Err(err) = result {
                                error!(error = %err, "xds client exited");
                            }
                        }
                        _ = config.changed() => {
                            current = config.borrow_and_update().clone();
                            reconnect_backoff = ReconnectBackoff::new(&current.connect_options.transport);
                            let _ = stop_tx.send(true);
                            let _ = (&mut run).await;
                            continue;
                        }
                        _ = shutdown.changed() => {
                            let _ = stop_tx.send(true);
                            let _ = (&mut run).await;
                            break;
                        }
                    }
                }
            }
            Err(err) => {
                stats.observe_connect_failure_with_error(err.to_string().as_str());
                error!(error = %err, "failed to connect to control plane");
            }
        }

        tokio::select! {
            _ = sleep(reconnect_backoff.next_delay()) => {}
            _ = config.changed() => {
                current = config.borrow_and_update().clone();
                reconnect_backoff = ReconnectBackoff::new(&current.connect_options.transport);
            }
            _ = shutdown.changed() => break,
        }
    }
}
