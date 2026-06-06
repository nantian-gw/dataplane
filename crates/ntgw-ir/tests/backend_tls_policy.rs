use ntgw_ir::Snapshot;
use ntgw_proto::gateway::control::v1 as proto;

#[test]
fn decodes_backend_tls_validation_from_proto() {
    let snapshot = Snapshot::from(proto::ConfigSnapshot {
        backends: vec![proto::BackendCluster {
            name: "orders:8443".to_string(),
            namespace: "default".to_string(),
            protocol: "HTTPS".to_string(),
            tls_validation: Some(proto::BackendTlsValidation {
                hostname: "orders.internal.example".to_string(),
                use_system_ca_certificates: true,
                ca_pems: vec!["PEM-A".to_string(), "PEM-B".to_string()],
                min_version: "TLS1_2".to_string(),
                max_version: "TLS1_3".to_string(),
                subject_alt_names: vec![
                    proto::BackendTlsSubjectAltName {
                        r#type: proto::BackendTlsSubjectAltNameType::Hostname.into(),
                        value: "orders.backend.svc".to_string(),
                    },
                    proto::BackendTlsSubjectAltName {
                        r#type: proto::BackendTlsSubjectAltNameType::Uri.into(),
                        value: "spiffe://cluster.local/ns/default/sa/orders".to_string(),
                    },
                ],
            }),
            wasm_plugin: None,
            ai_service: None,
            token_policy: None,
            ..Default::default()
        }],
        ..Default::default()
    });

    let policy = snapshot
        .backend_policy("default/orders:8443")
        .expect("backend policy");
    let validation = policy
        .tls_validation
        .as_ref()
        .expect("backend tls validation");

    assert_eq!(validation.hostname, "orders.internal.example");
    assert!(validation.use_system_ca_certificates);
    assert_eq!(validation.ca_pems, vec!["PEM-A", "PEM-B"]);
    assert_eq!(validation.min_version, "TLS1_2");
    assert_eq!(validation.max_version, "TLS1_3");
    assert_eq!(validation.subject_alt_names.len(), 2);
    assert_eq!(validation.subject_alt_names[0].kind, "Hostname");
    assert_eq!(validation.subject_alt_names[0].value, "orders.backend.svc");
    assert_eq!(validation.subject_alt_names[1].kind, "URI");
}
