use std::collections::BTreeMap;

use ntgw_observability::AccessLogMode;

#[test]
fn to_access_log_options_resolves_named_text_format() {
    let cfg = AccessLogConfig {
        mode: "text".to_string(),
        format: "%STATUS%".to_string(),
        format_name: "main".to_string(),
        formats: BTreeMap::from([("main".to_string(), "$remote_addr $status".to_string())]),
        ..AccessLogConfig::default()
    };

    let options = to_access_log_options(&cfg).expect("named format should resolve");

    assert_eq!(options.mode, AccessLogMode::Text);
    assert_eq!(options.format, "$remote_addr $status");
}

#[test]
fn to_access_log_options_falls_back_to_inline_format_when_name_is_empty() {
    let cfg = AccessLogConfig {
        mode: "text".to_string(),
        format: "$request_id $status".to_string(),
        format_name: String::new(),
        formats: BTreeMap::from([("main".to_string(), "$remote_addr $status".to_string())]),
        ..AccessLogConfig::default()
    };

    let options = to_access_log_options(&cfg).expect("inline format should remain active");

    assert_eq!(options.mode, AccessLogMode::Text);
    assert_eq!(options.format, "$request_id $status");
}

#[test]
fn to_access_log_options_rejects_unknown_named_text_format() {
    let cfg = AccessLogConfig {
        mode: "text".to_string(),
        format_name: "missing".to_string(),
        formats: BTreeMap::new(),
        ..AccessLogConfig::default()
    };

    let err = to_access_log_options(&cfg).expect_err("missing format should be rejected");
    assert!(err.to_string().contains("accessLog.formatName"));
    assert!(err.to_string().contains("missing"));
}

#[test]
fn to_access_log_options_ignores_named_format_validation_in_json_mode() {
    let cfg = AccessLogConfig {
        mode: "json".to_string(),
        format: "%EVENT%".to_string(),
        format_name: "missing".to_string(),
        formats: BTreeMap::new(),
        ..AccessLogConfig::default()
    };

    let options = to_access_log_options(&cfg).expect("json mode should ignore named formats");

    assert_eq!(options.mode, AccessLogMode::Json);
    assert_eq!(options.format, "%EVENT%");
}
