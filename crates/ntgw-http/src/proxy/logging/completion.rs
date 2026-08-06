use std::borrow::Cow;

use ntgw_observability::{
    AccessLogOptions, AccessLogRecord, AccessLogSampleKey, SharedTrafficStats,
    TrafficObservationRef, TrafficRuntimeIds, current_timestamp, emit_access_log,
    render_access_log, resolve_access_log_write_options,
};
use tracing::error;

use super::super::context::{
    RequestContext, clear_completed_request_context, saturating_latency_ms,
};
use super::super::request::access_log_route_annotations;
use super::super::retry::retry_completed_successfully;
use super::helpers::{build_request_line, extract_request_header};
use super::route_labels::request_route_labels;

pub(crate) fn observe_completed_request(
    access_log: &AccessLogOptions,
    traffic: &SharedTrafficStats,
    ctx: &mut RequestContext,
    latency_ms: u128,
    bytes_sent: usize,
) {
    if !ctx.is_mirror_subrequest {
        let route_labels = request_route_labels(ctx);
        let traffic_topology = ctx
            .selected_backend_config
            .as_ref()
            .map(|config| config.traffic_topology.as_ref())
            .or_else(|| {
                ctx.local_response_traffic_topology
                    .as_deref()
                    .map(|topology| topology.as_ref())
            });
        traffic.observe_ref_with_topology(
            TrafficObservationRef {
                listener_name: route_labels.listener_name,
                protocol: route_labels.effective_protocol(),
                route_namespace: route_labels.route_namespace,
                route_name: route_labels.route_name,
                route_kind: route_labels.route_kind,
                backend_name: route_labels.backend_name,
                status: Some(ctx.status),
                latency_ms: saturating_latency_ms(latency_ms),
                bytes_received: ctx.bytes_received as u64,
                bytes_sent: bytes_sent as u64,
                retry_attempts: ctx.retry_attempts,
                retried_success: retry_completed_successfully(ctx),
                upstream_pool_hits: ctx.upstream_pool_hits,
                upstream_pool_misses: ctx.upstream_pool_misses,
                upstream_peer_build_failures: ctx.upstream_peer_build_failures,
                upstream_connect_latency_ms: ctx.upstream_connect_latency_ms,
                upstream_connect_latency_ms_max: ctx.upstream_connect_latency_ms_max,
                upstream_connect_latency_ms_buckets: &ctx.upstream_connect_latency_ms_buckets,
                response_flags: ctx.response_flags.as_str(),
                runtime_ids: TrafficRuntimeIds {
                    listener: ctx.runtime_ids.listener.map(|id| id.as_u64()),
                    route: ctx.runtime_ids.route.map(|id| id.as_u64()),
                    backend: ctx.runtime_ids.backend.map(|id| id.as_u64()),
                },
            },
            traffic_topology,
        );
    }

    if !access_log.enabled || ctx.is_mirror_subrequest {
        clear_completed_request_context(ctx);
        return;
    }

    let route_annotations = access_log_route_annotations(ctx);
    let sample_key = access_log_sample_key(ctx);
    let Some(resolved_access_log) =
        resolve_access_log_write_options(access_log, route_annotations, &sample_key)
    else {
        clear_completed_request_context(ctx);
        return;
    };

    let write_result = {
        let route_labels = request_route_labels(ctx);
        let record = AccessLogRecord {
            event: "http_request".to_string(),
            timestamp: current_timestamp(),
            start_time_unix_ms: ctx.started_at_unix_ms,
            snapshot_version: ctx.snapshot_version.clone(),
            listener: Cow::Borrowed(route_labels.listener_name),
            listener_runtime_id: ctx.runtime_ids.listener.map(|id| id.to_string()),
            protocol: Cow::Borrowed(route_labels.effective_protocol()),
            client_ip: ctx.client_ip.clone(),
            host: ctx.host.clone(),
            method: ctx.method.clone(),
            path: ctx.path.clone(),
            request_id: ctx.request_id.clone(),
            route_namespace: Cow::Borrowed(route_labels.route_namespace),
            route_name: Cow::Borrowed(route_labels.route_name),
            route_kind: Cow::Borrowed(route_labels.route_kind),
            route_runtime_id: ctx.runtime_ids.route.map(|id| id.to_string()),
            rule_runtime_id: ctx.runtime_ids.rule.map(|id| id.to_string()),
            backend: Cow::Borrowed(route_labels.backend_name),
            backend_runtime_id: ctx.runtime_ids.backend.map(|id| id.to_string()),
            endpoint_runtime_id: ctx.runtime_ids.endpoint.map(|id| id.to_string()),
            status: Some(ctx.status),
            latency_ms,
            bytes_sent,
            bytes_received: ctx.bytes_received,
            retry_attempts: ctx.retry_attempts,
            response_flags: ctx.response_flags.clone(),
            request: build_request_line(ctx),
            http_version: ctx.http_version.clone(),
            query_string: ctx.query_string.clone(),
            referer: extract_request_header(ctx, "referer").into_owned(),
            user_agent: extract_request_header(ctx, "user-agent").into_owned(),
            x_forwarded_for: extract_request_header(ctx, "x-forwarded-for").into_owned(),
            upstream_addr: ctx.upstream_addr.clone(),
            upstream_connect_time_ms: ctx.upstream_connect_latency_ms as u128,
            content_type: ctx.response_content_type.clone(),
            connection_id: ctx.connection_id.clone(),
            request_header_values: ctx.access_log_request_headers.iter().map(|(k, v)| (k.to_string(), v.clone())).collect(),
            sent_response_header_values: ctx.access_log_sent_response_headers.iter().map(|(k, v)| (k.to_string(), v.clone())).collect(),
            upstream_response_header_values: ctx.access_log_upstream_response_headers.iter().map(|(k, v)| (k.to_string(), v.clone())).collect(),
            upstream_statuses: ctx.access_log_upstream_statuses.clone(),
            scheme: ctx.access_log_scheme.clone(),
            remote_port: ctx.access_log_remote_port,
        };
        render_access_log(&resolved_access_log, &record)
            .and_then(|line| emit_access_log(&resolved_access_log.path, &line))
    };
    if let Err(err) = write_result {
        error!(error = %err, "failed to emit access log");
    }

    clear_completed_request_context(ctx);
}

pub(crate) fn access_log_sample_key(ctx: &RequestContext) -> AccessLogSampleKey<'_> {
    let route_labels = request_route_labels(ctx);
    AccessLogSampleKey {
        event: "http_request",
        listener: route_labels.listener_name,
        listener_runtime_id: ctx.runtime_ids.listener.map(|id| id.as_u64()),
        request_id: ctx.request_id.as_str(),
        route_namespace: route_labels.route_namespace,
        route_name: route_labels.route_name,
        route_runtime_id: ctx.runtime_ids.route.map(|id| id.as_u64()),
        backend: route_labels.backend_name,
        backend_runtime_id: ctx.runtime_ids.backend.map(|id| id.as_u64()),
        start_time_unix_ms: ctx.started_at_unix_ms,
    }
}
