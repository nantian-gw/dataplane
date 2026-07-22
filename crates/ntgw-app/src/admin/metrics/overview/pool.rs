use super::super::prometheus::{append_counter, append_gauge};

pub(super) fn append_stream_pool_metrics(out: &mut String) {
    let Some(snap) = ntgw_stream::pool::global_pool_snapshot() else {
        return;
    };

    append_gauge(
        out,
        "nantian_gateway_dataplane_stream_pool_active",
        "Current number of active TCP stream connections held by callers (not in idle pool).",
        snap.active_connections,
    );
    append_gauge(
        out,
        "nantian_gateway_dataplane_stream_pool_idle",
        "Current number of idle TCP stream connections held in the pool ready for reuse.",
        snap.idle_connections,
    );
    append_gauge(
        out,
        "nantian_gateway_dataplane_stream_pool_peak_active",
        "Peak number of active TCP stream connections observed since the pool was created.",
        snap.peak_active_connections,
    );
    append_counter(
        out,
        "nantian_gateway_dataplane_stream_pool_hits_total",
        "Cumulative number of successful TCP stream connection pool reuses.",
        snap.connection_hits,
    );
    append_counter(
        out,
        "nantian_gateway_dataplane_stream_pool_misses_total",
        "Cumulative number of TCP stream connections created (pool misses).",
        snap.connection_misses,
    );
}
