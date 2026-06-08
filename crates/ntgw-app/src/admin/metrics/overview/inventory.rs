use super::super::{
    context::MetricsContext,
    prometheus::{append_gauge, append_labeled_gauge_map},
};

pub(super) fn append_inventory_metrics(out: &mut String, ctx: &MetricsContext) {
    let snapshot = &ctx.snapshot;
    let overload = &ctx.overload;
    let session_persistence = &ctx.session_persistence;

    append_gauge(
        out,
        "nantian_gateway_dataplane_ready",
        "1 if the dataplane readiness check currently passes, 0 otherwise.",
        ctx.ready,
    );
    append_gauge(
        out,
        "nantian_gateway_dataplane_listener_count",
        "Total number of listeners in the active snapshot.",
        snapshot.listeners.len() as u64,
    );
    append_gauge(
        out,
        "nantian_gateway_dataplane_http_route_count",
        "Total number of HTTPRoute resources in the active snapshot.",
        snapshot.http_routes.len() as u64,
    );
    append_gauge(
        out,
        "nantian_gateway_dataplane_grpc_route_count",
        "Total number of GRPCRoute resources in the active snapshot.",
        snapshot.grpc_routes.len() as u64,
    );
    append_gauge(
        out,
        "nantian_gateway_dataplane_stream_route_count",
        "Total number of TCPRoute, UDPRoute, and TLSRoute resources in the active snapshot.",
        snapshot.stream_routes.len() as u64,
    );
    append_gauge(
        out,
        "nantian_gateway_dataplane_backend_count",
        "Total number of backend clusters in the active snapshot.",
        snapshot.backends.len() as u64,
    );
    append_gauge(
        out,
        "nantian_gateway_dataplane_secret_count",
        "Total number of TLS secrets in the active snapshot.",
        snapshot.secrets.len() as u64,
    );
    append_gauge(
        out,
        "nantian_gateway_dataplane_http3_configured",
        "1 if HTTP/3 is configured for the dataplane, 0 otherwise.",
        ctx.http3_configured,
    );
    append_gauge(
        out,
        "nantian_gateway_dataplane_http3_available",
        "1 if the current Nantian build exposes HTTP/3 support, 0 otherwise.",
        ctx.http3_available,
    );
    append_gauge(
        out,
        "nantian_gateway_dataplane_http3_enabled",
        "1 if HTTP/3 is both configured and available, 0 otherwise.",
        ctx.http3_enabled,
    );
    append_gauge(
        out,
        "nantian_gateway_dataplane_session_persistence_active",
        "1 if the active snapshot contains any route or backend policy using session persistence, 0 otherwise.",
        ctx.session_persistence_active,
    );
    append_gauge(
        out,
        "nantian_gateway_dataplane_session_persistence_ephemeral_secret",
        "1 if the dataplane is using a generated per-process session persistence secret, 0 otherwise.",
        ctx.session_persistence_ephemeral,
    );
    append_gauge(
        out,
        "nantian_gateway_dataplane_session_persistence_route_rule_count",
        "Number of HTTPRoute and GRPCRoute rules in the active snapshot that configure session persistence.",
        session_persistence.route_rules as u64,
    );
    append_gauge(
        out,
        "nantian_gateway_dataplane_session_persistence_backend_policy_count",
        "Number of backend policies in the active snapshot that configure session persistence.",
        session_persistence.backend_policies as u64,
    );
    append_gauge(
        out,
        "nantian_gateway_dataplane_http_global_inflight_current",
        "Current number of admitted HTTP, HTTPS, and gRPC requests counted against the global inflight budget.",
        overload.http_global_inflight_current,
    );
    append_labeled_gauge_map(
        out,
        "nantian_gateway_dataplane_http_listener_inflight_current",
        "Current number of admitted HTTP, HTTPS, and gRPC requests counted against each listener inflight budget.",
        "listener",
        &overload.http_listener_inflight_current,
        &ctx.http_listener_metric_labels,
    );
    append_labeled_gauge_map(
        out,
        "nantian_gateway_dataplane_http_route_inflight_current",
        "Current number of admitted HTTP, HTTPS, and gRPC requests counted against each route inflight budget.",
        "route",
        &overload.http_route_inflight_current,
        &ctx.route_metric_labels,
    );
    append_gauge(
        out,
        "nantian_gateway_dataplane_tcp_global_connections_current",
        "Current number of admitted TCP and TLS passthrough sessions counted against the global connection budget.",
        overload.tcp_global_connections_current,
    );
    append_labeled_gauge_map(
        out,
        "nantian_gateway_dataplane_tcp_listener_connections_current",
        "Current number of admitted TCP and TLS passthrough sessions counted against each listener connection budget.",
        "listener",
        &overload.tcp_listener_connections_current,
        &ctx.tcp_listener_metric_labels,
    );
    append_gauge(
        out,
        "nantian_gateway_dataplane_udp_global_datagrams_current",
        "Current number of admitted UDP datagrams counted against the global datagram budget.",
        overload.udp_global_datagrams_current,
    );
    append_labeled_gauge_map(
        out,
        "nantian_gateway_dataplane_udp_listener_datagrams_current",
        "Current number of admitted UDP datagrams counted against each listener datagram budget.",
        "listener",
        &overload.udp_listener_datagrams_current,
        &ctx.udp_listener_metric_labels,
    );
}
