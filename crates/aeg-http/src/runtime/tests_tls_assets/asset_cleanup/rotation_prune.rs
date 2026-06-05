#[test]
fn materialized_tls_assets_prune_stale_files_after_rotation() {
    let asset_dir = std::env::temp_dir()
        .join("aether-gateway")
        .join("http-listeners")
        .join(super::listener_plan::unique_asset_dir_name());

    let original = ListenerPlan {
        listeners: vec![PlannedListener {
            name: "default/gw/https".to_string(),
            bind: "127.0.0.1:443".to_string(),
            protocol: ListenerProtocol::Tls(single_tls_material(
                "default/example-cert",
                "CERT-A",
                "KEY-A",
                Some("CA-OLD"),
            )),
        }],
    };
    let rotated = ListenerPlan {
        listeners: vec![PlannedListener {
            name: "default/gw/https".to_string(),
            bind: "127.0.0.1:443".to_string(),
            protocol: ListenerProtocol::Tls(single_tls_material(
                "default/example-cert",
                "CERT-B",
                "KEY-B",
                None,
            )),
        }],
    };

    super::listener_plan::materialize_tls_assets_in_dir(&original, &asset_dir)
        .expect("write original assets");
    super::listener_plan::materialize_tls_assets_in_dir(&rotated, &asset_dir)
        .expect("write rotated assets");
    let referenced = super::listener_plan::referenced_tls_asset_prefixes(&rotated);
    super::listener_plan::cleanup_unused_tls_assets_in_dir(&asset_dir, &referenced)
        .expect("cleanup stale assets");

    let entries = fs::read_dir(&asset_dir)
        .expect("read dir")
        .map(|entry| entry.expect("dir entry").path())
        .collect::<Vec<_>>();
    let file_names = entries
        .iter()
        .filter_map(|path| path.file_name().and_then(|item| item.to_str()))
        .collect::<Vec<_>>();

    fs::remove_dir_all(&asset_dir).expect("cleanup asset dir");

    assert_eq!(
        file_names.len(),
        2,
        "unexpected remaining files: {file_names:?}"
    );
    assert!(file_names.iter().any(|name| name.ends_with(".crt")));
    assert!(file_names.iter().any(|name| name.ends_with(".key")));
    assert!(!file_names.iter().any(|name| name.ends_with(".ca.crt")));
}
