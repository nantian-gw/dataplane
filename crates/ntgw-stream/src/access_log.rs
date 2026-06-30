use ntgw_ir::SelectedBackend;
use ntgw_observability::{AccessLogOptions, access_log_enabled_for_route, epoch_millis};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct StreamAccessLogState {
    pub(crate) snapshot_version: String,
    pub(crate) started_at_unix_ms: u128,
}

pub(crate) fn stream_access_log_state(
    access_log: &AccessLogOptions,
    selected: &SelectedBackend,
    snapshot_id: &str,
) -> Option<StreamAccessLogState> {
    access_log_enabled_for_route(access_log, &selected.route_annotations).then(|| {
        StreamAccessLogState {
            snapshot_version: snapshot_id.to_string(),
            started_at_unix_ms: epoch_millis(),
        }
    })
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use ntgw_ir::{BackendEndpoint, RouteKind, SelectedBackend};
    use ntgw_observability::AccessLogOptions;

    use super::stream_access_log_state;

    #[test]
    fn stream_access_log_state_skips_disabled_routes() {
        let selected = sample_selected_backend(BTreeMap::new());
        let state = stream_access_log_state(
            &AccessLogOptions {
                enabled: false,
                ..AccessLogOptions::default()
            },
            &selected,
            "snapshot-1",
        );

        assert!(state.is_none());
    }

    #[test]
    fn stream_access_log_state_honors_route_enable_annotation() {
        let selected = sample_selected_backend(BTreeMap::from([(
            "gateway.nantian.dev/access-log-enabled".to_string(),
            "true".to_string(),
        )]));
        let state = stream_access_log_state(
            &AccessLogOptions {
                enabled: false,
                ..AccessLogOptions::default()
            },
            &selected,
            "snapshot-1",
        )
        .expect("route annotation should enable stream access log");

        assert_eq!(state.snapshot_version, "snapshot-1");
        assert!(state.started_at_unix_ms > 0);
    }

    fn sample_selected_backend(route_annotations: BTreeMap<String, String>) -> SelectedBackend {
        SelectedBackend {
            route_policy: None,
            route_kind: RouteKind::Tcp,
            route_name: "tcp-route".to_string(),
            route_namespace: "default".to_string(),
            rule_index: None,
            route_annotations,
            listener_name: "default/gw/tcp".to_string(),
            listener_protocol: "TCP".to_string(),
            backend: BackendEndpoint {
                address: "127.0.0.1".to_string(),
                port: 8080,
                healthy: true,
            },
            backend_name: "default/backend:8080".to_string(),
            filters: Vec::new(),
            matched_http_path: None,
            timeouts: None,
            retry: None,
            session_persistence: None,
            backend_tls: None,
        }
    }
}
