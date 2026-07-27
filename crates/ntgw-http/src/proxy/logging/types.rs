#[derive(Clone, Copy)]
pub(crate) struct RequestRouteLabels<'a> {
    pub(crate) listener_name: &'a str,
    pub(crate) listener_protocol: &'a str,
    pub(crate) route_namespace: &'a str,
    pub(crate) route_name: &'a str,
    pub(crate) route_kind: &'a str,
    pub(crate) backend_name: &'a str,
}

impl<'a> RequestRouteLabels<'a> {
    pub(crate) fn effective_protocol(&self) -> &'a str {
        if !self.listener_protocol.is_empty() {
            return self.listener_protocol;
        }

        if self.route_kind.eq_ignore_ascii_case("grpc") {
            "GRPC"
        } else {
            "HTTP"
        }
    }
}
