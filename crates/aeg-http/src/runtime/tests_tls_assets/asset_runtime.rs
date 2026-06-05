#[test]
fn runtime_tls_asset_root_uses_configured_directory() {
    let runtime = RuntimeOptions {
        tls_asset_dir: "/var/lib/nantian-gw/tls-assets".to_string(),
        ..RuntimeOptions::default()
    };

    assert_eq!(
        super::tls_asset_root(&runtime),
        PathBuf::from("/var/lib/nantian-gw/tls-assets")
    );
}

#[cfg(unix)]
#[test]
fn materialized_tls_assets_use_owner_only_permissions() {
    use std::os::unix::fs::PermissionsExt;

    let asset_dir = std::env::temp_dir()
        .join("nantian-gw")
        .join("http-listeners")
        .join(super::listener_plan::unique_asset_dir_name());
    let plan = ListenerPlan {
        listeners: vec![PlannedListener {
            name: "default/gw/https".to_string(),
            bind: "127.0.0.1:443".to_string(),
            protocol: ListenerProtocol::Tls(single_tls_material(
                "default/example-cert",
                "CERT-A",
                "KEY-A",
                Some("CA-A"),
            )),
        }],
    };

    super::listener_plan::materialize_tls_assets_in_dir(&plan, &asset_dir)
        .expect("write tls assets");
    let runtime_plan = super::listener_plan::materialize_runtime_plan(&plan, &asset_dir);
    let RuntimeListenerProtocol::Tls {
        cert_path,
        key_path,
        client_ca_path,
        ..
    } = &runtime_plan.listeners[0].protocol
    else {
        panic!("expected tls runtime listener");
    };

    let dir_mode = fs::metadata(&asset_dir)
        .expect("asset dir metadata")
        .permissions()
        .mode()
        & 0o777;
    assert_eq!(dir_mode, 0o700);

    for path in [
        PathBuf::from(cert_path),
        PathBuf::from(key_path),
        PathBuf::from(client_ca_path.as_ref().expect("client ca path should exist")),
    ] {
        let file_mode = fs::metadata(&path)
            .expect("asset metadata")
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(file_mode, 0o600, "unexpected mode for {}", path.display());
    }

    fs::remove_dir_all(&asset_dir).expect("cleanup asset dir");
}

#[test]
fn skips_tls_listener_without_primary_identity_when_materializing_runtime_plan() {
    let asset_dir = std::env::temp_dir()
        .join("nantian-gw")
        .join("http-listeners")
        .join(super::listener_plan::unique_asset_dir_name());
    let plan = ListenerPlan {
        listeners: vec![PlannedListener {
            name: "default/gw/https".to_string(),
            bind: "127.0.0.1:443".to_string(),
            protocol: ListenerProtocol::Tls(super::TlsMaterial {
                identities: Vec::new(),
                min_version: "1.2".to_string(),
                max_version: "1.3".to_string(),
                client_ca_bundle_pem: None,
                frontend_validation_mode: None,
            }),
        }],
    };

    let runtime_plan = super::listener_plan::materialize_runtime_plan(&plan, &asset_dir);
    assert!(runtime_plan.listeners.is_empty());

    let referenced = super::listener_plan::referenced_tls_asset_prefixes(&plan);
    assert!(referenced.is_empty());
}
