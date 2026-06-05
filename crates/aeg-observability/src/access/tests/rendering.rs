#[test]
fn renders_access_log_template() {
    let line = render_access_log(
        &AccessLogOptions {
            mode: AccessLogMode::Text,
            ..AccessLogOptions::default()
        },
        &AccessLogRecord {
            event: "http_request".to_string(),
            start_time_unix_ms: 123,
            listener: "edge".to_string(),
            protocol: "HTTP".to_string(),
            host: "example.com".to_string(),
            method: "GET".to_string(),
            status: Some(200),
            bytes_sent: 128,
            ..AccessLogRecord::default()
        },
    )
    .expect("line should render");

    assert!(line.contains("http_request"));
    assert!(line.contains("edge"));
    assert!(line.contains("HTTP"));
    assert!(line.contains("example.com"));
    assert!(line.contains("GET"));
    assert!(line.contains("200"));
    assert!(line.contains("128"));
}

#[test]
fn parses_access_log_template_into_literals_and_placeholders() {
    let parts = parse_access_log_template("prefix %EVENT% middle %UNKNOWN% suffix %STATUS%");

    assert_eq!(
        parts,
        vec![
            AccessLogTemplatePart::Literal("prefix ".to_string()),
            AccessLogTemplatePart::Placeholder(AccessLogPlaceholder::Event),
            AccessLogTemplatePart::Literal(" middle %UNKNOWN% suffix ".to_string()),
            AccessLogTemplatePart::Placeholder(AccessLogPlaceholder::Status),
        ]
    );
}

#[test]
fn renders_access_log_template_with_repeated_placeholders_and_unknown_tokens() {
    let line = render_access_log_template(
        "%EVENT% %UNKNOWN% %EVENT% %STATUS%",
        &AccessLogRecord {
            event: "http_request".to_string(),
            status: Some(201),
            ..AccessLogRecord::default()
        },
    );

    assert_eq!(line, "http_request %UNKNOWN% http_request 201");
}

#[test]
fn defaults_to_json_access_logs() {
    let options = AccessLogOptions::default();
    assert_eq!(options.mode, AccessLogMode::Json);
    assert_eq!(options.path, "stdout");
}

#[test]
fn renders_json_access_log() {
    let line = render_access_log(
        &AccessLogOptions {
            mode: AccessLogMode::Json,
            ..AccessLogOptions::default()
        },
        &AccessLogRecord {
            event: "tcp_session".to_string(),
            listener: "stream".to_string(),
            protocol: "TCP".to_string(),
            ..AccessLogRecord::default()
        },
    )
    .expect("json should render");

    assert!(line.contains("\"event\":\"tcp_session\""));
    assert!(line.contains("\"listener\":\"stream\""));
}

#[test]
fn renders_runtime_id_access_log_fields() {
    let record = AccessLogRecord {
        listener_runtime_id: Some("listener-1".to_string()),
        route_runtime_id: Some("route-1".to_string()),
        rule_runtime_id: Some("rule-1".to_string()),
        backend_runtime_id: Some("backend-1".to_string()),
        endpoint_runtime_id: Some("endpoint-1".to_string()),
        ..AccessLogRecord::default()
    };

    let json = render_access_log(
        &AccessLogOptions {
            mode: AccessLogMode::Json,
            ..AccessLogOptions::default()
        },
        &record,
    )
    .expect("json should render");
    assert!(json.contains("\"listenerRuntimeId\":\"listener-1\""));
    assert!(json.contains("\"routeRuntimeId\":\"route-1\""));
    assert!(json.contains("\"ruleRuntimeId\":\"rule-1\""));
    assert!(json.contains("\"backendRuntimeId\":\"backend-1\""));
    assert!(json.contains("\"endpointRuntimeId\":\"endpoint-1\""));

    let text = render_access_log(
        &AccessLogOptions {
            mode: AccessLogMode::Text,
            format: "%LISTENER_RUNTIME_ID% %ROUTE_RUNTIME_ID% %RULE_RUNTIME_ID% %BACKEND_RUNTIME_ID% %ENDPOINT_RUNTIME_ID%".to_string(),
            ..AccessLogOptions::default()
        },
        &record,
    )
    .expect("text should render");
    assert_eq!(text, "listener-1 route-1 rule-1 backend-1 endpoint-1");
}
