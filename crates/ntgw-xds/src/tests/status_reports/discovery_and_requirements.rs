#[test]
fn discovery_requests_encode_ack_and_nack_status() {
    let ack = discovery_ack("dp-1", "kind", "v1", "nonce-1", &supported_features());
    assert_eq!(ack.result_status, DiscoveryResultStatus::DiscoveryAck as i32);
    assert_eq!(ack.error_detail, "");

    let nack = discovery_nack(
        "dp-1",
        "kind",
        "v2",
        "nonce-2",
        "listener reload failed",
        &supported_features(),
    );
    assert_eq!(nack.result_status, DiscoveryResultStatus::DiscoveryNack as i32);
    assert_eq!(nack.error_detail, "listener reload failed");
}

#[test]
fn discovery_messages_include_supported_features() {
    run_discovery_messages_include_supported_features();
}

pub(super) fn run_discovery_messages_include_supported_features() {
    let supported = canonicalize_supported_features([
        " backend.wasm_plugin.v1 ",
        "route.labels.v1",
        "",
        "core.v1",
        "route.labels.v1",
    ]);

    let open = discovery_open("dp-1", "kind", "v1", &supported);
    let ack = discovery_ack("dp-1", "kind", "v1", "nonce-1", &supported);
    let nack = discovery_nack("dp-1", "kind", "v1", "nonce-1", "nope", &supported);

    assert_eq!(open.supported_features, supported);
    assert_eq!(ack.supported_features, supported);
    assert_eq!(nack.supported_features, supported);
}

#[test]
fn snapshot_apply_requirements_match_listener_protocols() {
    let snapshot = Snapshot {
        listeners: vec![
            Listener {
                protocol: "LISTENER_PROTOCOL_HTTPS".to_string(),
                ..Listener::default()
            },
            Listener {
                protocol: "LISTENER_PROTOCOL_TLS_PASSTHROUGH".to_string(),
                ..Listener::default()
            },
            Listener {
                protocol: "LISTENER_PROTOCOL_TCP".to_string(),
                ..Listener::default()
            },
        ],
        ..Snapshot::default()
    };

    let requirements = snapshot_runtime_apply_requirements(&snapshot);
    assert_eq!(
        requirements,
        RuntimeApplyRequirements {
            http: false,
            tls: true,
            stream: true,
        }
    );
}

#[test]
fn required_runtime_names_describes_combined_requirements() {
    assert_eq!(
        required_runtime_names(RuntimeApplyRequirements {
            http: true,
            tls: true,
            stream: true,
        }),
        "HTTP, TLS, and stream"
    );
    assert_eq!(
        required_runtime_names(RuntimeApplyRequirements {
            http: false,
            tls: true,
            stream: true,
        }),
        "TLS and stream"
    );
    assert_eq!(
        required_runtime_names(RuntimeApplyRequirements {
            http: false,
            tls: true,
            stream: false,
        }),
        "TLS"
    );
    assert_eq!(
        required_runtime_names(RuntimeApplyRequirements {
            http: false,
            tls: false,
            stream: true,
        }),
        "stream"
    );
}

#[test]
fn preflight_required_features_reports_sorted_missing_features() {
    run_preflight_required_features_reports_sorted_missing_features();
}

pub(super) fn run_preflight_required_features_reports_sorted_missing_features() {
    let snapshot = ConfigSnapshot {
        required_features: vec![
            "route.labels.v1".to_string(),
            "backend.wasm_plugin.v1".to_string(),
        ],
        ..ConfigSnapshot::default()
    };
    let supported = canonicalize_supported_features(["core.v1"]);

    let result = preflight_required_features(&snapshot, &supported);

    assert_eq!(
        result,
        Err(
            "snapshot requires unsupported features: backend.wasm_plugin.v1, route.labels.v1"
                .to_string()
        )
    );
}

#[test]
fn preflight_required_features_accepts_supported_snapshot() {
    run_preflight_required_features_accepts_supported_snapshot();
}

pub(super) fn run_preflight_required_features_accepts_supported_snapshot() {
    let snapshot = ConfigSnapshot {
        required_features: vec!["core.v1".to_string(), "route.labels.v1".to_string()],
        ..ConfigSnapshot::default()
    };

    assert_eq!(
        preflight_required_features(&snapshot, &supported_features()),
        Ok(())
    );
}
