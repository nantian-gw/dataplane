#[test]
fn materialize_tls_assets_cleans_up_stale_temp_files() {
    let asset_dir = std::env::temp_dir()
        .join("nantian-gw")
        .join("http-listeners")
        .join(super::listener_plan::unique_asset_dir_name());
    fs::create_dir_all(&asset_dir).expect("create asset dir");
    fs::write(asset_dir.join(".ntgw-tls-asset-tmp-stale-cert"), "stale")
        .expect("write stale cert temp");
    fs::write(asset_dir.join(".ntgw-tls-asset-tmp-stale-key"), "stale")
        .expect("write stale key temp");

    let plan = ListenerPlan {
        listeners: vec![PlannedListener {
            name: "default/gw/https".to_string().into(),
            bind: "127.0.0.1:443".to_string(),
            protocol: ListenerProtocol::Tls(single_tls_material(
                "default/example-cert",
                "CERT-A",
                "KEY-A",
                None,
            )),
        }],
    };

    super::listener_plan::materialize_tls_assets_in_dir(&plan, &asset_dir)
        .expect("write tls assets");
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

    assert_eq!(file_names.len(), 2, "unexpected files: {file_names:?}");
    assert!(
        file_names
            .iter()
            .all(|name| !name.starts_with(".ntgw-tls-asset-tmp-")),
        "unexpected temp files: {file_names:?}"
    );

    fs::remove_dir_all(&asset_dir).expect("cleanup asset dir");
}
