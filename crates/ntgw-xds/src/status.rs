use std::time::{Instant, SystemTime};

use ntgw_ir::{SharedSnapshot, Snapshot};
use ntgw_observability::{RuntimeStatsSnapshot, SharedRuntimeStats};
use ntgw_proto::gateway::control::v1::{DiscoveryRequest, DiscoveryResultStatus, StatusReport};

use crate::TransportOptions;

pub(crate) const SNAPSHOT_APPLIED_MESSAGE: &str = "snapshot applied";
pub(crate) const SNAPSHOT_REJECTED_MESSAGE_PREFIX: &str = "snapshot rejected: ";
pub(crate) const WAITING_FOR_SNAPSHOT_MESSAGE: &str = "waiting for snapshot";

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct RuntimeApplyRequirements {
    pub(crate) http: bool,
    pub(crate) tls: bool,
    pub(crate) stream: bool,
}

pub(crate) fn build_status_report(
    node_id: &str,
    snapshot: &SharedSnapshot,
    runtime: &SharedRuntimeStats,
    include_version: bool,
) -> StatusReport {
    let current = snapshot.read();
    let runtime = runtime.snapshot();
    let rejection_message = current_runtime_rejection_message(&runtime, current.id.as_str());
    let ready = if rejection_message.is_some() {
        runtime_has_last_good_configuration(&runtime)
    } else {
        !current.id.is_empty()
    };
    let message = if let Some(rejection_message) = rejection_message {
        format!("{SNAPSHOT_REJECTED_MESSAGE_PREFIX}{rejection_message}")
    } else if ready {
        SNAPSHOT_APPLIED_MESSAGE.to_string()
    } else {
        WAITING_FOR_SNAPSHOT_MESSAGE.to_string()
    };

    StatusReport {
        node_id: node_id.to_string(),
        version: if include_version {
            current.id.clone()
        } else {
            String::new()
        },
        ready,
        message,
        observed_at: Some(SystemTime::now().into()),
    }
}

pub(crate) fn snapshot_runtime_apply_requirements(snapshot: &Snapshot) -> RuntimeApplyRequirements {
    RuntimeApplyRequirements {
        http: snapshot
            .listeners
            .iter()
            .any(|listener| requires_http_runtime(listener.protocol.as_str())),
        tls: snapshot
            .listeners
            .iter()
            .any(|listener| requires_tls_runtime(listener.protocol.as_str())),
        stream: snapshot
            .listeners
            .iter()
            .any(|listener| requires_stream_runtime(listener.protocol.as_str())),
    }
}

pub(crate) async fn wait_for_runtime_apply_result(
    runtime: SharedRuntimeStats,
    version: &str,
    requirements: RuntimeApplyRequirements,
    transport: &TransportOptions,
) -> std::result::Result<(), String> {
    if !requirements.http && !requirements.tls && !requirements.stream {
        return Ok(());
    }

    let deadline = Instant::now() + transport.apply_timeout;
    let mut apply_events = runtime.subscribe_apply_events();
    loop {
        if let Some(result) =
            current_runtime_apply_result(&runtime.snapshot(), version, requirements)
        {
            return result;
        }
        if Instant::now() >= deadline {
            return Err(format!(
                "timed out waiting for {} runtime apply result for snapshot {version}",
                required_runtime_names(requirements),
            ));
        }

        let wait = deadline.saturating_duration_since(Instant::now());
        tokio::time::timeout(wait, apply_events.changed())
            .await
            .map_err(|_| {
                format!(
                    "timed out waiting for {runtime_names} runtime apply result for snapshot {version}",
                    runtime_names = required_runtime_names(requirements),
                )
            })?
            .map_err(|_| {
                format!(
                    "timed out waiting for {runtime_names} runtime apply result for snapshot {version}",
                    runtime_names = required_runtime_names(requirements),
                )
            })?;
    }
}

pub(crate) fn required_runtime_names(requirements: RuntimeApplyRequirements) -> &'static str {
    match (requirements.http, requirements.tls, requirements.stream) {
        (true, true, true) => "HTTP, TLS, and stream",
        (true, true, false) => "HTTP and TLS",
        (true, false, true) => "HTTP and stream",
        (false, true, true) => "TLS and stream",
        (true, false, false) => "HTTP",
        (false, true, false) => "TLS",
        (false, false, true) => "stream",
        (false, false, false) => "listener",
    }
}

pub(crate) fn discovery_ack(
    node_id: &str,
    cluster: &str,
    version: &str,
    nonce: &str,
) -> DiscoveryRequest {
    DiscoveryRequest {
        node_id: node_id.to_string(),
        cluster: cluster.to_string(),
        version: version.to_string(),
        nonce: nonce.to_string(),
        subscriptions: vec!["*".to_string()],
        result_status: DiscoveryResultStatus::Ack as i32,
        error_detail: String::new(),
    }
}

pub(crate) fn discovery_nack(
    node_id: &str,
    cluster: &str,
    version: &str,
    nonce: &str,
    error_detail: &str,
) -> DiscoveryRequest {
    DiscoveryRequest {
        node_id: node_id.to_string(),
        cluster: cluster.to_string(),
        version: version.to_string(),
        nonce: nonce.to_string(),
        subscriptions: vec!["*".to_string()],
        result_status: DiscoveryResultStatus::Nack as i32,
        error_detail: error_detail.to_string(),
    }
}

fn current_runtime_rejection_message(
    runtime: &RuntimeStatsSnapshot,
    version: &str,
) -> Option<String> {
    if version.is_empty() {
        return None;
    }

    let http = (runtime.http_last_reload_failure_version == version
        && !runtime.http_last_reload_failure_message.is_empty())
    .then(|| runtime.http_last_reload_failure_message.clone());
    let tls = (runtime.tls_last_reload_failure_version == version
        && !runtime.tls_last_reload_failure_message.is_empty())
    .then(|| runtime.tls_last_reload_failure_message.clone());
    let stream = (runtime.stream_last_reload_failure_version == version
        && !runtime.stream_last_reload_failure_message.is_empty())
    .then(|| runtime.stream_last_reload_failure_message.clone());

    match (http, tls, stream) {
        (None, None, None) => None,
        (Some(http), None, None) => Some(http),
        (None, Some(tls), None) => Some(tls),
        (None, None, Some(stream)) => Some(stream),
        (http, tls, stream) => {
            let mut segments = Vec::new();
            if let Some(http) = http {
                segments.push(format!("HTTP runtime: {http}"));
            }
            if let Some(tls) = tls {
                segments.push(format!("TLS runtime: {tls}"));
            }
            if let Some(stream) = stream {
                segments.push(format!("stream runtime: {stream}"));
            }
            Some(segments.join("; "))
        }
    }
}

fn runtime_has_last_good_configuration(runtime: &RuntimeStatsSnapshot) -> bool {
    !runtime.http_last_good_reload_version.is_empty()
        || !runtime.tls_last_good_reload_version.is_empty()
        || !runtime.stream_last_good_reload_version.is_empty()
}

fn requires_http_runtime(protocol: &str) -> bool {
    matches!(
        protocol,
        "LISTENER_PROTOCOL_HTTP"
            | "LISTENER_PROTOCOL_HTTP3"
            | "LISTENER_PROTOCOL_GRPC"
            | "HTTP"
            | "HTTP3"
            | "GRPC"
    )
}

fn requires_tls_runtime(protocol: &str) -> bool {
    matches!(
        protocol,
        "LISTENER_PROTOCOL_HTTPS"
            | "LISTENER_PROTOCOL_TLS_PASSTHROUGH"
            | "HTTPS"
            | "TLS"
            | "TLS_PASSTHROUGH"
    )
}

fn requires_stream_runtime(protocol: &str) -> bool {
    matches!(
        protocol,
        "LISTENER_PROTOCOL_TCP" | "LISTENER_PROTOCOL_UDP" | "TCP" | "UDP"
    )
}

fn current_runtime_apply_result(
    snapshot: &RuntimeStatsSnapshot,
    version: &str,
    requirements: RuntimeApplyRequirements,
) -> Option<std::result::Result<(), String>> {
    let http_ready = !requirements.http || snapshot.http_last_good_reload_version == version;
    let tls_ready = !requirements.tls || snapshot.tls_last_good_reload_version == version;
    let stream_ready = !requirements.stream || snapshot.stream_last_good_reload_version == version;
    if http_ready && tls_ready && stream_ready {
        return Some(Ok(()));
    }
    if requirements.http
        && snapshot.http_last_reload_failure_version == version
        && !snapshot.http_last_reload_failure_message.is_empty()
    {
        return Some(Err(format!(
            "HTTP runtime apply failed: {message}",
            message = snapshot.http_last_reload_failure_message
        )));
    }
    if requirements.tls
        && snapshot.tls_last_reload_failure_version == version
        && !snapshot.tls_last_reload_failure_message.is_empty()
    {
        return Some(Err(format!(
            "TLS runtime apply failed: {message}",
            message = snapshot.tls_last_reload_failure_message
        )));
    }
    if requirements.stream
        && snapshot.stream_last_reload_failure_version == version
        && !snapshot.stream_last_reload_failure_message.is_empty()
    {
        return Some(Err(format!(
            "stream runtime apply failed: {message}",
            message = snapshot.stream_last_reload_failure_message
        )));
    }

    None
}
