#[test]
fn materialized_tls_assets_write_expected_contents_without_temp_files() {
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
    security_policy: None,
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
    let file_names = fs::read_dir(&asset_dir)
        .expect("read dir")
        .map(|entry| {
            entry
                .expect("dir entry")
                .file_name()
                .into_string()
                .expect("utf8 file name")
        })
        .collect::<Vec<_>>();

    assert_eq!(fs::read_to_string(cert_path).expect("read cert"), "CERT-A");
    assert_eq!(fs::read_to_string(key_path).expect("read key"), "KEY-A");
    assert_eq!(
        fs::read_to_string(
            client_ca_path
                .as_ref()
                .expect("client ca path should exist")
        )
        .expect("read ca"),
        "CA-A"
    );
    assert_eq!(file_names.len(), 3, "unexpected files: {file_names:?}");
    assert!(
        file_names
            .iter()
            .all(|name| !name.starts_with(".ntgw-tls-asset-tmp-")),
        "unexpected temp files: {file_names:?}"
    );

    fs::remove_dir_all(&asset_dir).expect("cleanup asset dir");
}
