use std::sync::Arc;
use std::time::Duration;

use ntgw_ir::{BackendEndpoint, SharedSnapshot, Snapshot};
use tokio::{
    net::TcpStream,
    sync::watch,
    task::{JoinHandle, JoinSet},
    time::{MissedTickBehavior, interval, timeout},
};
use tracing::warn;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ProbeTarget {
    pub backend_name: String,
    pub address: String,
    pub port: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ProbeResult {
    pub target: ProbeTarget,
    pub healthy: bool,
}

#[derive(Clone, Debug)]
pub(crate) struct ReloadableProbeConfig {
    pub(crate) enabled: bool,
    pub(crate) probe_interval: Duration,
    pub(crate) probe_timeout: Duration,
    pub(crate) unhealthy_threshold: u32,
}

pub(crate) fn spawn(
    snapshot: SharedSnapshot,
    config: watch::Receiver<std::sync::Arc<ReloadableProbeConfig>>,
    shutdown: watch::Receiver<bool>,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        run(snapshot, config, shutdown).await;
    })
}

async fn run(
    snapshot: SharedSnapshot,
    mut config: watch::Receiver<std::sync::Arc<ReloadableProbeConfig>>,
    mut shutdown: watch::Receiver<bool>,
) {
    let mut current = config.borrow().clone();
    let mut ticker = interval(current.probe_interval.max(Duration::from_millis(1)));
    ticker.set_missed_tick_behavior(MissedTickBehavior::Delay);

    loop {
        if !current.enabled {
            tokio::select! {
                _ = config.changed() => {
                    current = config.borrow_and_update().clone();
                    ticker = interval(current.probe_interval.max(Duration::from_millis(1)));
                    ticker.set_missed_tick_behavior(MissedTickBehavior::Delay);
                    continue;
                }
                _ = shutdown.changed() => break,
            }
        }

        tokio::select! {
            _ = ticker.tick() => {}
            _ = config.changed() => {
                current = config.borrow_and_update().clone();
                ticker = interval(current.probe_interval.max(Duration::from_millis(1)));
                ticker.set_missed_tick_behavior(MissedTickBehavior::Delay);
                continue;
            }
            _ = shutdown.changed() => break,
        }

        let targets = {
            let current = snapshot.load();
            collect_probe_targets(&current)
        };
        if targets.is_empty() {
            continue;
        }

        let mut probes = JoinSet::new();
        for target in targets {
            let probe_timeout = current.probe_timeout;
            probes.spawn(async move {
                let healthy = probe_target_once(&target, probe_timeout).await;
                ProbeResult { target, healthy }
            });
        }

        let mut results = Vec::new();
        while let Some(joined) = probes.join_next().await {
            match joined {
                Ok(result) => results.push(result),
                Err(err) => warn!(error = %err, "active health probe task failed"),
            }
        }

        if results.is_empty() {
            continue;
        }

        let unhealthy_threshold = current.unhealthy_threshold;
        let mut current_snapshot = Snapshot::clone(&snapshot.load());
        apply_probe_results(&mut current_snapshot, &results, unhealthy_threshold);
        snapshot.store(Arc::new(current_snapshot));
    }
}

pub(crate) fn collect_probe_targets(snapshot: &Snapshot) -> Vec<ProbeTarget> {
    let mut targets = Vec::new();

    for cluster in &snapshot.backends {
        if backend_uses_udp(cluster.protocol.as_ref()) {
            continue;
        }

        let backend_name = format!("{}/{}", cluster.namespace, cluster.name);
        for endpoint in &cluster.endpoints {
            if !endpoint.healthy {
                continue;
            }

            targets.push(ProbeTarget {
                backend_name: backend_name.clone(),
                address: endpoint.address.clone(),
                port: endpoint.port,
            });
        }
    }

    targets
}

pub(crate) async fn probe_target_once(target: &ProbeTarget, probe_timeout: Duration) -> bool {
    let Ok(port) = u16::try_from(target.port) else {
        return false;
    };

    matches!(
        timeout(
            probe_timeout,
            TcpStream::connect((target.address.as_str(), port))
        )
        .await,
        Ok(Ok(_))
    )
}

pub(crate) fn apply_probe_results(
    snapshot: &mut Snapshot,
    results: &[ProbeResult],
    unhealthy_threshold: u32,
) {
    for result in results {
        let Some(endpoint) = find_endpoint(
            snapshot,
            result.target.backend_name.as_str(),
            result.target.address.as_str(),
            result.target.port,
        ) else {
            continue;
        };

        if result.healthy {
            snapshot.record_endpoint_active_probe_success(
                result.target.backend_name.as_str(),
                &endpoint,
            );
        } else {
            snapshot.record_endpoint_active_probe_failure(
                result.target.backend_name.as_str(),
                &endpoint,
                unhealthy_threshold,
            );
        }
    }
}

fn find_endpoint(
    snapshot: &Snapshot,
    backend_name: &str,
    address: &str,
    port: u32,
) -> Option<BackendEndpoint> {
    snapshot.backends.iter().find_map(|cluster| {
        let candidate_backend_name = format!("{}/{}", cluster.namespace, cluster.name);
        (candidate_backend_name == backend_name).then(|| {
            cluster
                .endpoints
                .iter()
                .find(|endpoint| endpoint.address == address && endpoint.port == port)
                .cloned()
        })?
    })
}

fn backend_uses_udp(protocol: &str) -> bool {
    protocol.eq_ignore_ascii_case("UDP") || protocol.ends_with("_UDP")
}

#[cfg(test)]
mod tests;
