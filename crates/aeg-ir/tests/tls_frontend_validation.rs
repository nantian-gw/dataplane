use aeg_ir::Snapshot;
use aeg_proto::gateway::control::v1 as proto;

#[test]
fn decodes_listener_frontend_validation_from_proto() {
    let snapshot = Snapshot::from(proto::ConfigSnapshot {
        listeners: vec![proto::Listener {
            name: "default/gw/https".to_string(),
            tls: Some(proto::TlsConfig {
                enabled: true,
                frontend_validation: Some(proto::FrontendValidation {
                    ca_pems: vec!["CA-PEM".to_string()],
                    mode: "AllowInsecureFallback".to_string(),
                }),
                ..Default::default()
            }),
            ..Default::default()
        }],
        ..Default::default()
    });

    let validation = snapshot.listeners[0]
        .tls
        .as_ref()
        .and_then(|tls| tls.frontend_validation.as_ref())
        .expect("frontend validation");
    assert_eq!(validation.ca_pems, vec!["CA-PEM".to_string()]);
    assert_eq!(validation.mode, "AllowInsecureFallback".to_string());
}
