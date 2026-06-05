use aeg_ir::RouteKind;
use aeg_observability::UPSTREAM_CONNECT_LATENCY_MS_BUCKET_COUNT;

pub(crate) const ZERO_UPSTREAM_CONNECT_LATENCY_MS_BUCKETS: [u32;
    UPSTREAM_CONNECT_LATENCY_MS_BUCKET_COUNT] = [0; UPSTREAM_CONNECT_LATENCY_MS_BUCKET_COUNT];

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct StreamPoolCounters {
    pub(crate) hits: u32,
    pub(crate) misses: u32,
}

impl StreamPoolCounters {}

pub(crate) fn stream_route_kind_label(route_kind: &RouteKind) -> &'static str {
    match route_kind {
        RouteKind::Http => "Http",
        RouteKind::Grpc => "Grpc",
        RouteKind::Tcp => "Tcp",
        RouteKind::Udp => "Udp",
        RouteKind::Tls => "Tls",
    }
}

#[cfg(test)]
mod tests {
    use aeg_ir::RouteKind;

    use super::stream_route_kind_label;

    #[test]
    fn stream_route_kind_label_matches_existing_debug_labels() {
        assert_eq!(stream_route_kind_label(&RouteKind::Tcp), "Tcp");
        assert_eq!(stream_route_kind_label(&RouteKind::Udp), "Udp");
        assert_eq!(stream_route_kind_label(&RouteKind::Tls), "Tls");
    }
}
