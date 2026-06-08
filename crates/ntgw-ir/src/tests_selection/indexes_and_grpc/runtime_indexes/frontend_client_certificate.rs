use crate::{FrontendValidation, TlsConfig};

#[test]
fn listener_frontend_client_certificate_lookup_uses_runtime_index_semantics() {
    let mut snapshot = Snapshot {
        listeners: vec![
            Listener {
                name: "plain".to_string(),
                ..Listener::default()
            },
            Listener {
                name: "reject".to_string(),
                tls: Some(TlsConfig {
                    frontend_validation: Some(FrontendValidation {
                        mode: "RejectClientCertificate".to_string(),
                        ..FrontendValidation::default()
                    }),
                    ..TlsConfig::default()
                }),
                ..Listener::default()
            },
            Listener {
                name: "strict".to_string(),
                tls: Some(TlsConfig {
                    frontend_validation: Some(FrontendValidation {
                        ca_pems: vec!["CA".to_string()],
                        ..FrontendValidation::default()
                    }),
                    ..TlsConfig::default()
                }),
                ..Listener::default()
            },
            Listener {
                name: "fallback".to_string(),
                tls: Some(TlsConfig {
                    frontend_validation: Some(FrontendValidation {
                        ca_pems: vec!["CA".to_string()],
                        mode: "AllowInsecureFallback".to_string(),
                    }),
                    ..TlsConfig::default()
                }),
                ..Listener::default()
            },
        ],
        ..Snapshot::default()
    };

    snapshot.rebuild_runtime_indexes();

    assert!(!snapshot.listener_requires_frontend_client_certificate_close("plain", false));
    assert!(snapshot.listener_requires_frontend_client_certificate_close("reject", false));
    assert!(snapshot.listener_requires_frontend_client_certificate_close("reject", true));
    assert!(snapshot.listener_requires_frontend_client_certificate_close("strict", false));
    assert!(!snapshot.listener_requires_frontend_client_certificate_close("strict", true));
    assert!(!snapshot.listener_requires_frontend_client_certificate_close("fallback", false));
    assert!(!snapshot.listener_requires_frontend_client_certificate_close("missing", false));

    snapshot.listeners[2].tls = None;
    snapshot.rebuild_runtime_indexes();

    assert!(!snapshot.listener_requires_frontend_client_certificate_close("strict", false));
}
