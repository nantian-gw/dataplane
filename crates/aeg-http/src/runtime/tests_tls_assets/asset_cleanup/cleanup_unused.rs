#[test]
fn cleanup_unused_tls_assets_removes_temp_files_but_keeps_referenced_assets() {
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
                None,
            )),
        }],
    };

    super::listener_plan::materialize_tls_assets_in_dir(&plan, &asset_dir)
        .expect("write tls assets");
    fs::write(asset_dir.join(".aeg-tls-asset-tmp-stale-cert"), "stale").expect("write stale temp");

    let referenced = super::listener_plan::referenced_tls_asset_prefixes(&plan);
    super::listener_plan::cleanup_unused_tls_assets_in_dir(&asset_dir, &referenced)
        .expect("cleanup unused assets");
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
            .all(|name| !name.starts_with(".aeg-tls-asset-tmp-")),
        "unexpected temp files: {file_names:?}"
    );
    assert!(file_names.iter().any(|name| name.ends_with(".crt")));
    assert!(file_names.iter().any(|name| name.ends_with(".key")));

    fs::remove_dir_all(&asset_dir).expect("cleanup asset dir");
}
