#[test]
fn discovery_requests_encode_ack_and_nack_status() {
    let ack = discovery_ack("dp-1", "kind", "v1", "nonce-1");
    assert_eq!(ack.result_status, DiscoveryResultStatus::Ack as i32);
    assert_eq!(ack.error_detail, "");

    let nack = discovery_nack("dp-1", "kind", "v2", "nonce-2", "listener reload failed");
    assert_eq!(nack.result_status, DiscoveryResultStatus::Nack as i32);
    assert_eq!(nack.error_detail, "listener reload failed");
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
