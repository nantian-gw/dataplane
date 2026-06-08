use serde_json::{Value, json};

use super::{
    super::{
        AppState,
        filters::{is_http_listener, is_https_listener, is_stream_listener},
    },
    listener_signals::ListenerSignalSummary,
    overview_builder::SummaryValueInputs,
};

mod warnings;
pub(super) use self::warnings::{WarningData, build_warning_data};

pub(super) struct SummaryOverviewSections {
    pub(super) meta_overview: Value,
    pub(super) instance_overview: Value,
    pub(super) health_overview: Value,
    pub(super) warning_overview: Value,
    pub(super) snapshot_overview: Value,
    pub(super) runtime_overview: Value,
    pub(super) resource_overview: Value,
    pub(super) feature_overview: Value,
    pub(super) xds_overview: Value,
    pub(super) traffic_overview: Value,
    pub(super) overload_overview: Value,
    pub(super) summary_overviews: Value,
}

#[allow(clippy::too_many_arguments)]
pub(super) fn build_overview_sections(
    state: &AppState,
    inputs: &SummaryValueInputs<'_>,
    warnings: &WarningData,
    listener_signals: &ListenerSignalSummary,
    http3_available: bool,
    http3_enabled: bool,
) -> SummaryOverviewSections {
    let config = state.current_config();
    let snapshot_overview = json!({
        "schemaVersion": 1,
        "summary": {
            "version": inputs.snapshot.id,
            "status": inputs.current_snapshot.status,
            "servingLastGoodSnapshot": inputs.current_snapshot.serving_last_good_snapshot,
            "lastGoodSnapshotVersion": inputs.current_snapshot.last_good_snapshot_version,
            "fallbackState": inputs.current_snapshot.fallback_state,
            "rejected": inputs.current_snapshot.rejected,
            "rejectionRuntime": inputs.current_snapshot.rejection_runtime,
        },
        "snapshotVersion": inputs.snapshot.id,
        "current": {
            "status": inputs.current_snapshot.status,
            "accepted": inputs.current_snapshot.accepted,
            "rejected": inputs.current_snapshot.rejected,
        },
        "serving": {
            "lastGoodSnapshot": inputs.current_snapshot.serving_last_good_snapshot,
            "lastGoodSnapshotVersion": inputs.current_snapshot.last_good_snapshot_version,
            "fallbackState": inputs.current_snapshot.fallback_state,
        },
        "rejection": {
            "version": inputs.current_snapshot.rejection_version,
            "runtime": inputs.current_snapshot.rejection_runtime,
            "message": inputs.current_snapshot.rejection_message,
        }
    });
    let runtime_overview = json!({
        "schemaVersion": 1,
        "summary": {
            "required": {
                "http": inputs.http_runtime.required,
                "tls": inputs.tls_runtime.required,
                "stream": inputs.stream_runtime.required,
            },
            "currentStatuses": {
                "http": inputs.http_runtime.status,
                "tls": inputs.tls_runtime.status,
                "stream": inputs.stream_runtime.status,
            },
            "acceptedPlanes": usize::from(inputs.http_runtime.accepted)
                + usize::from(inputs.tls_runtime.accepted)
                + usize::from(inputs.stream_runtime.accepted),
            "rejectedPlanes": usize::from(inputs.http_runtime.rejected)
                + usize::from(inputs.tls_runtime.rejected)
                + usize::from(inputs.stream_runtime.rejected),
        },
        "http": {
            "required": inputs.http_runtime.required,
            "status": inputs.http_runtime.status,
            "accepted": inputs.http_runtime.accepted,
            "rejected": inputs.http_runtime.rejected,
            "stats": {
                "running": inputs.runtime.http_runtime_running,
                "lastExitUnixSeconds": inputs.runtime.http_last_exit_unix_seconds,
                "lastExitMessage": inputs.runtime.http_last_exit_message,
                "listenerReloadFailures": inputs.runtime.http_listener_reload_failures,
                "lastReloadAttemptVersion": inputs.runtime.http_last_reload_attempt_version,
                "lastGoodReloadVersion": inputs.runtime.http_last_good_reload_version,
                "lastReloadFailureVersion": inputs.runtime.http_last_reload_failure_version,
                "lastReloadFailureListener": inputs.runtime.http_last_reload_failure_listener,
                "lastReloadFailureMessage": inputs.runtime.http_last_reload_failure_message,
                "currentFailures": inputs.runtime.http_current_failures,
                "tlsAssetReuses": inputs.runtime.http_tls_asset_reuses,
            }
        },
        "tls": {
            "required": inputs.tls_runtime.required,
            "status": inputs.tls_runtime.status,
            "accepted": inputs.tls_runtime.accepted,
            "rejected": inputs.tls_runtime.rejected,
            "stats": {
                "running": inputs.runtime.tls_runtime_running,
                "lastExitUnixSeconds": inputs.runtime.tls_last_exit_unix_seconds,
                "lastExitMessage": inputs.runtime.tls_last_exit_message,
                "listenerReloadFailures": inputs.runtime.tls_listener_reload_failures,
                "lastReloadAttemptVersion": inputs.runtime.tls_last_reload_attempt_version,
                "lastGoodReloadVersion": inputs.runtime.tls_last_good_reload_version,
                "lastReloadFailureVersion": inputs.runtime.tls_last_reload_failure_version,
                "lastReloadFailureListener": inputs.runtime.tls_last_reload_failure_listener,
                "lastReloadFailureMessage": inputs.runtime.tls_last_reload_failure_message,
                "currentFailures": inputs.runtime.tls_current_failures,
            }
        },
        "stream": {
            "required": inputs.stream_runtime.required,
            "status": inputs.stream_runtime.status,
            "accepted": inputs.stream_runtime.accepted,
            "rejected": inputs.stream_runtime.rejected,
            "stats": {
                "running": inputs.runtime.stream_runtime_running,
                "lastExitUnixSeconds": inputs.runtime.stream_last_exit_unix_seconds,
                "lastExitMessage": inputs.runtime.stream_last_exit_message,
                "listenerReloadFailures": inputs.runtime.stream_listener_reload_failures,
                "lastReloadAttemptVersion": inputs.runtime.stream_last_reload_attempt_version,
                "lastGoodReloadVersion": inputs.runtime.stream_last_good_reload_version,
                "lastReloadFailureVersion": inputs.runtime.stream_last_reload_failure_version,
                "lastReloadFailureListener": inputs.runtime.stream_last_reload_failure_listener,
                "lastReloadFailureMessage": inputs.runtime.stream_last_reload_failure_message,
                "currentFailures": inputs.runtime.stream_current_failures,
            }
        }
    });
    let resource_overview = json!({
        "schemaVersion": 1,
        "counts": {
            "listeners": {
                "total": inputs.snapshot.listeners.len(),
                "http": inputs.snapshot.listeners.iter().filter(|listener| is_http_listener(&listener.protocol)).count(),
                "https": inputs.snapshot.listeners.iter().filter(|listener| is_https_listener(&listener.protocol)).count(),
                "stream": inputs.snapshot.listeners.iter().filter(|listener| is_stream_listener(&listener.protocol)).count(),
            },
            "routes": {
                "total": inputs.snapshot.http_routes.len() + inputs.snapshot.grpc_routes.len() + inputs.snapshot.stream_routes.len(),
                "http": inputs.snapshot.http_routes.len(),
                "grpc": inputs.snapshot.grpc_routes.len(),
                "stream": inputs.snapshot.stream_routes.len(),
            },
            "backends": inputs.snapshot.backends.len(),
            "secrets": inputs.snapshot.secrets.len(),
        },
        "listeners": {
            "total": inputs.snapshot.listeners.len(),
            "http": inputs.snapshot.listeners.iter().filter(|listener| is_http_listener(&listener.protocol)).count(),
            "https": inputs.snapshot.listeners.iter().filter(|listener| is_https_listener(&listener.protocol)).count(),
            "stream": inputs.snapshot.listeners.iter().filter(|listener| is_stream_listener(&listener.protocol)).count(),
        },
        "routes": {
            "total": inputs.snapshot.http_routes.len() + inputs.snapshot.grpc_routes.len() + inputs.snapshot.stream_routes.len(),
            "http": inputs.snapshot.http_routes.len(),
            "grpc": inputs.snapshot.grpc_routes.len(),
            "stream": inputs.snapshot.stream_routes.len(),
        },
        "backends": inputs.snapshot.backends.len(),
        "secrets": inputs.snapshot.secrets.len(),
    });
    let feature_overview = json!({
        "schemaVersion": 1,
        "http3": {
            "status": {
                "configured": config.http3_configured,
                "available": http3_available,
                "enabled": http3_enabled,
            },
            "configured": config.http3_configured,
            "available": http3_available,
            "enabled": http3_enabled,
        },
        "sessionPersistence": {
            "status": {
                "configured": !config.session_persistence_uses_ephemeral_secret,
                "usesEphemeralSecret": config.session_persistence_uses_ephemeral_secret,
                "active": inputs.session_persistence.active(),
            },
            "counts": {
                "routeRuleCount": inputs.session_persistence.route_rules,
                "backendPolicyCount": inputs.session_persistence.backend_policies,
            },
            "configured": !config.session_persistence_uses_ephemeral_secret,
            "usesEphemeralSecret": config.session_persistence_uses_ephemeral_secret,
            "active": inputs.session_persistence.active(),
            "routeRuleCount": inputs.session_persistence.route_rules,
            "backendPolicyCount": inputs.session_persistence.backend_policies,
        }
    });
    let xds_overview = json!({
        "schemaVersion": 1,
        "connection": {
            "counts": {
                "connectFailures": inputs.xds.connect_failures,
                "streamFailures": inputs.xds.stream_failures,
            },
            "status": {
                "streamConnected": inputs.xds.stream_connected,
                "lastControlPlaneContactUnixSeconds": inputs.xds.last_control_plane_contact_unix_seconds,
                "lastConnectError": inputs.xds.last_connect_error,
                "lastConnectFailureUnixSeconds": inputs.xds.last_connect_failure_unix_seconds,
                "lastStreamError": inputs.xds.last_stream_error,
                "lastStreamFailureUnixSeconds": inputs.xds.last_stream_failure_unix_seconds,
            },
            "streamConnected": inputs.xds.stream_connected,
            "lastControlPlaneContactUnixSeconds": inputs.xds.last_control_plane_contact_unix_seconds,
            "connectFailures": inputs.xds.connect_failures,
            "streamFailures": inputs.xds.stream_failures,
            "lastConnectError": inputs.xds.last_connect_error,
            "lastConnectFailureUnixSeconds": inputs.xds.last_connect_failure_unix_seconds,
            "lastStreamError": inputs.xds.last_stream_error,
            "lastStreamFailureUnixSeconds": inputs.xds.last_stream_failure_unix_seconds,
        },
        "connectFailures": inputs.xds.connect_failures,
        "streamFailures": inputs.xds.stream_failures,
        "streamConnected": inputs.xds.stream_connected,
        "lastControlPlaneContactUnixSeconds": inputs.xds.last_control_plane_contact_unix_seconds,
        "lastConnectError": inputs.xds.last_connect_error,
        "lastConnectFailureUnixSeconds": inputs.xds.last_connect_failure_unix_seconds,
        "lastStreamError": inputs.xds.last_stream_error,
        "lastStreamFailureUnixSeconds": inputs.xds.last_stream_failure_unix_seconds,
        "snapshots": {
            "counts": {
                "applied": inputs.xds.snapshots_applied,
                "nacked": inputs.xds.snapshots_nacked,
                "skipped": inputs.xds.snapshots_skipped,
            },
            "status": {
                "lastSnapshotVersion": inputs.xds.last_snapshot_version,
                "lastNackVersion": inputs.xds.last_nack_version,
                "lastNackMessage": inputs.xds.last_nack_message,
                "lastApplyUnixSeconds": inputs.xds.last_apply_unix_seconds,
            },
            "applied": inputs.xds.snapshots_applied,
            "nacked": inputs.xds.snapshots_nacked,
            "skipped": inputs.xds.snapshots_skipped,
            "lastSnapshotVersion": inputs.xds.last_snapshot_version,
            "lastNackVersion": inputs.xds.last_nack_version,
            "lastNackMessage": inputs.xds.last_nack_message,
            "lastApplyUnixSeconds": inputs.xds.last_apply_unix_seconds,
        }
    });
    let traffic_overview = json!({
        "schemaVersion": 1,
        "summary": {
            "counts": {
                "totalEvents": inputs.traffic.total_events,
                "retriedSuccessEvents": inputs.traffic.total_retried_success_events,
                "retriedEvents": inputs.traffic.total_retried_events,
                "retryAttempts": inputs.traffic.total_retry_attempts,
                "bytesReceived": inputs.traffic.total_bytes_received,
                "bytesSent": inputs.traffic.total_bytes_sent,
                "upstreamPoolHits": inputs.traffic.total_upstream_pool_hits,
                "upstreamPoolMisses": inputs.traffic.total_upstream_pool_misses,
            },
            "status": {
                "retryRate": inputs.retry_rate,
                "failoverSuccessRate": inputs.failover_success_rate,
                "upstreamPoolHitRatio": inputs.upstream_pool_hit_ratio,
                "maxLatencyMs": inputs.traffic.max_latency_ms,
                "upstreamConnectAvgMs": inputs.upstream_connect_latency_avg_ms,
                "upstreamConnectMaxMs": inputs.traffic.max_upstream_connect_latency_ms,
            }
        },
        "events": {
            "total": inputs.traffic.total_events,
            "retriedSuccess": inputs.traffic.total_retried_success_events,
            "retried": inputs.traffic.total_retried_events,
            "retryAttempts": inputs.traffic.total_retry_attempts,
        },
        "bytes": {
            "received": inputs.traffic.total_bytes_received,
            "sent": inputs.traffic.total_bytes_sent,
        },
        "rates": {
            "retry": inputs.retry_rate,
            "failoverSuccess": inputs.failover_success_rate,
            "upstreamPoolHitRatio": inputs.upstream_pool_hit_ratio,
        },
        "latencyMs": {
            "max": inputs.traffic.max_latency_ms,
            "upstreamConnectAvg": inputs.upstream_connect_latency_avg_ms,
            "upstreamConnectMax": inputs.traffic.max_upstream_connect_latency_ms,
        },
        "upstreamPool": {
            "hits": inputs.traffic.total_upstream_pool_hits,
            "misses": inputs.traffic.total_upstream_pool_misses,
        }
    });
    let overload_overview = json!({
        "schemaVersion": 1,
        "http": {
            "current": {
                "globalInflight": inputs.overload.http_global_inflight_current,
                "listenerInflight": inputs.http_listener_inflight_current,
                "routeInflight": inputs.http_route_inflight_current,
                "listenerInflightByName": inputs.overload.http_listener_inflight_current.clone(),
                "routeInflightByName": inputs.overload.http_route_inflight_current.clone(),
            },
            "rejected": {
                "total": inputs.overload.http_rejected_total,
                "global": inputs.overload.http_rejected_global_total,
                "listener": inputs.overload.http_rejected_listener_total,
                "route": inputs.overload.http_rejected_route_total,
                "listenerByName": inputs.overload.http_rejected_listener_by_name.clone(),
                "routeByName": inputs.overload.http_rejected_route_by_name.clone(),
            },
        },
        "tcp": {
            "current": {
                "globalConnections": inputs.overload.tcp_global_connections_current,
                "listenerConnections": inputs.tcp_listener_connections_current,
                "listenerConnectionsByName": inputs.overload.tcp_listener_connections_current.clone(),
            },
            "rejected": {
                "total": inputs.overload.tcp_rejected_total,
                "global": inputs.overload.tcp_rejected_global_total,
                "listener": inputs.overload.tcp_rejected_listener_total,
                "listenerByName": inputs.overload.tcp_rejected_listener_by_name.clone(),
            },
        },
        "udp": {
            "current": {
                "globalDatagrams": inputs.overload.udp_global_datagrams_current,
                "listenerDatagrams": inputs.udp_listener_datagrams_current,
                "listenerDatagramsByName": inputs.overload.udp_listener_datagrams_current.clone(),
            },
            "rejected": {
                "total": inputs.overload.udp_rejected_total,
                "global": inputs.overload.udp_rejected_global_total,
                "listener": inputs.overload.udp_rejected_listener_total,
                "listenerByName": inputs.overload.udp_rejected_listener_by_name.clone(),
            },
        },
    });
    let health_overview = json!({
        "schemaVersion": 1,
        "status": {
            "ready": inputs.readiness.ready,
            "snapshotStatus": inputs.current_snapshot.status,
            "readinessState": inputs.readiness.state,
            "readinessReason": inputs.readiness.reason,
        },
        "warnings": {
            "count": warnings.messages.len(),
            "hasWarnings": !warnings.messages.is_empty(),
            "primaryCategory": warnings.primary_category.clone(),
        },
        "ready": inputs.readiness.ready,
        "readinessState": inputs.readiness.state,
        "readinessReason": inputs.readiness.reason,
        "warningCount": warnings.messages.len(),
        "hasWarnings": !warnings.messages.is_empty(),
        "primaryWarningCategory": warnings.primary_category.clone(),
        "snapshotStatus": inputs.current_snapshot.status,
        "runtime": {
            "http": inputs.http_runtime.status,
            "tls": inputs.tls_runtime.status,
            "stream": inputs.stream_runtime.status,
        }
    });
    let warning_overview = json!({
        "schemaVersion": 1,
        "status": {
            "count": warnings.messages.len(),
            "hasWarnings": !warnings.messages.is_empty(),
            "primaryCategory": warnings.primary_category.clone(),
            "primaryMessage": warnings.primary_message.clone(),
        },
        "count": warnings.messages.len(),
        "hasWarnings": !warnings.messages.is_empty(),
        "categories": warnings.categories.clone(),
        "counts": warnings.counts.clone(),
        "messages": warnings.messages.clone(),
    });
    let instance_overview = json!({
        "schemaVersion": 1,
        "identity": {
            "nodeId": config.node_id,
            "cluster": config.cluster,
        },
        "snapshot": {
            "ready": inputs.readiness.ready,
            "version": inputs.snapshot.id,
            "status": inputs.current_snapshot.status,
            "readinessState": inputs.readiness.state,
        },
        "nodeId": config.node_id,
        "cluster": config.cluster,
        "ready": inputs.readiness.ready,
        "readinessState": inputs.readiness.state,
        "snapshotVersion": inputs.snapshot.id,
        "snapshotStatus": inputs.current_snapshot.status,
    });
    let meta_overview = json!({
        "schemaVersion": 1,
        "surface": "dataplane-summary",
        "handshake": {
            "surface": "dataplane-summary",
            "summarySchemaVersion": 1,
        },
        "overviewKeys": [
            "meta",
            "instance",
            "health",
            "warnings",
            "snapshot",
            "runtime",
            "resources",
            "features",
            "xds",
            "traffic",
            "overload",
            "listenerState",
            "listenerSignals",
        ],
        "overviewSchemas": {
            "summaryOverviews": 1,
            "instance": 1,
            "health": 1,
            "warnings": 1,
            "snapshot": 1,
            "runtime": 1,
            "resources": 1,
            "features": 1,
            "xds": 1,
            "traffic": 1,
            "overload": 1,
            "listenerState": 1,
            "listenerSignals": 1,
        }
    });
    let summary_overviews = json!({
        "schemaVersion": 1,
        "overviewKeys": [
            "meta",
            "instance",
            "health",
            "warnings",
            "snapshot",
            "runtime",
            "resources",
            "features",
            "xds",
            "traffic",
            "overload",
            "listenerState",
            "listenerSignals",
        ],
        "meta": meta_overview.clone(),
        "instance": instance_overview.clone(),
        "health": health_overview.clone(),
        "warnings": warning_overview.clone(),
        "snapshot": snapshot_overview.clone(),
        "runtime": runtime_overview.clone(),
        "resources": resource_overview.clone(),
        "features": feature_overview.clone(),
        "xds": xds_overview.clone(),
        "traffic": traffic_overview.clone(),
        "overload": overload_overview.clone(),
        "listenerState": listener_signals.listener_state_overview.clone(),
        "listenerSignals": {
            "schemaVersion": 1,
            "overviewKeys": [
                "bundle",
                "state",
            ],
            "bundle": listener_signals.listener_overviews.clone(),
            "state": listener_signals.listener_state_overview.clone(),
        }
    });

    SummaryOverviewSections {
        meta_overview,
        instance_overview,
        health_overview,
        warning_overview,
        snapshot_overview,
        runtime_overview,
        resource_overview,
        feature_overview,
        xds_overview,
        traffic_overview,
        overload_overview,
        summary_overviews,
    }
}
