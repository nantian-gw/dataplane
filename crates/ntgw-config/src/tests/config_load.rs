use std::fs;

use crate::DataPlaneConfig;

const MINIMAL_CONFIG_YAML: &str = r#"
node_id: dp
cluster: kind
control_plane_addr: http://127.0.0.1:18080
admin_addr: 127.0.0.1:19080
"#;

#[test]
fn parse_yaml_parses_minimal_config() {
    let cfg =
        DataPlaneConfig::parse_yaml(MINIMAL_CONFIG_YAML).expect("minimal config should parse");
    assert_eq!(cfg.node_id, "dp");
    assert_eq!(cfg.cluster, "kind");
}

#[test]
fn parse_yaml_rejects_malformed_document() {
    let err = DataPlaneConfig::parse_yaml("node_id: [unterminated").unwrap_err();
    assert!(!format!("{err}").is_empty());
}

#[test]
fn load_reports_path_context_for_missing_file() {
    let dir = super::tempfile_dir();
    let missing = dir.join("does-not-exist.yaml");

    let err = DataPlaneConfig::load(&missing).unwrap_err();
    let rendered = format!("{err:#}");

    assert!(
        rendered.contains("reading dataplane config file"),
        "error should mention the read stage: {rendered}"
    );
    assert!(
        rendered.contains(&missing.display().to_string()),
        "error should mention the file path: {rendered}"
    );

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn load_reports_path_context_for_malformed_file() {
    let dir = super::tempfile_dir();
    let path = dir.join("bad-config.yaml");
    fs::write(&path, "node_id: [unterminated").expect("malformed config should be written");

    let err = DataPlaneConfig::load(&path).unwrap_err();
    let rendered = format!("{err:#}");

    assert!(
        rendered.contains("parsing dataplane config file"),
        "error should mention the parse stage: {rendered}"
    );
    assert!(
        rendered.contains(&path.display().to_string()),
        "error should mention the file path: {rendered}"
    );

    let _ = fs::remove_dir_all(&dir);
}
