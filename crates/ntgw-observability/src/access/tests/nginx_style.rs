use std::collections::BTreeSet;

use super::template::{
    AccessLogVariable, CompiledAccessLogTemplatePart, compile_access_log_template,
};

#[test]
fn parses_mixed_legacy_and_nginx_style_tokens() {
    let compiled =
        compile_access_log_template(r#"%STATUS% $remote_addr ${request} "$http_user_agent""#);

    assert_eq!(
        compiled.requirements.request_headers,
        BTreeSet::from(["user-agent".to_string()])
    );
    assert!(compiled.requirements.uses_request_line);
    assert!(compiled.parts.iter().any(|part| matches!(
        part,
        CompiledAccessLogTemplatePart::LegacyPlaceholder(AccessLogPlaceholder::Status)
    )));
    assert!(compiled.parts.iter().any(|part| matches!(
        part,
        CompiledAccessLogTemplatePart::Variable(AccessLogVariable::RemoteAddr)
    )));
    assert!(compiled.parts.iter().any(|part| matches!(
        part,
        CompiledAccessLogTemplatePart::Variable(AccessLogVariable::Request)
    )));
}

#[test]
fn renders_nginx_style_text_access_log() {
    let line = render_access_log(
        &AccessLogOptions {
            mode: AccessLogMode::Text,
            format: r#"$remote_addr "$request" $status $request_time "$http_user_agent""#
                .to_string(),
            ..AccessLogOptions::default()
        },
        &AccessLogRecord {
            event: "http_request".to_string(),
            timestamp: "2026-06-16T00:00:00.000Z".to_string(),
            client_ip: "192.0.2.10".to_string(),
            method: "GET".to_string(),
            path: "/orders".to_string(),
            query_string: "id=1".to_string(),
            request: "GET /orders?id=1 HTTP/2".to_string(),
            request_id: "req-1".to_string(),
            http_version: "HTTP/2".to_string(),
            status: Some(200),
            latency_ms: 123,
            upstream_addr: "10.0.0.10:8080".to_string(),
            request_header_values: BTreeMap::from([(
                "user-agent".to_string(),
                "curl/8.7.1".to_string(),
            )]),
            ..AccessLogRecord::default()
        },
    )
    .expect("line should render");

    assert_eq!(
        line,
        r#"192.0.2.10 "GET /orders?id=1 HTTP/2" 200 0.123 "curl/8.7.1""#
    );
}

#[test]
fn unknown_nginx_style_variables_stay_literal() {
    let line = render_access_log(
        &AccessLogOptions {
            mode: AccessLogMode::Text,
            format: "$unknown_var %UNKNOWN%".to_string(),
            ..AccessLogOptions::default()
        },
        &AccessLogRecord::default(),
    )
    .expect("line should render");

    assert_eq!(line, "$unknown_var %UNKNOWN%");
}

#[test]
fn stream_records_render_http_only_nginx_variables_as_dash() {
    let line = render_access_log(
        &AccessLogOptions {
            mode: AccessLogMode::Text,
            format: "$request_method $args $http_user_agent $ntgw_event".to_string(),
            ..AccessLogOptions::default()
        },
        &AccessLogRecord {
            event: "tcp_session".to_string(),
            protocol: Cow::Borrowed("TCP"),
            ..AccessLogRecord::default()
        },
    )
    .expect("line should render");

    assert_eq!(line, "- - - tcp_session");
}

#[test]
fn parses_response_side_nginx_variables_into_requirements() {
    let compiled = compile_access_log_template(
        r#"$sent_http_content_type $upstream_http_server $upstream_status $scheme $remote_port"#,
    );

    assert_eq!(
        compiled.requirements.sent_response_headers,
        BTreeSet::from(["content-type".to_string()])
    );
    assert_eq!(
        compiled.requirements.upstream_response_headers,
        BTreeSet::from(["server".to_string()])
    );
    assert!(compiled.requirements.needs_upstream_status);
    assert!(compiled.requirements.needs_scheme);
    assert!(compiled.requirements.needs_remote_port);
}

#[test]
fn renders_response_side_nginx_variables() {
    let line = render_access_log(
        &AccessLogOptions {
            mode: AccessLogMode::Text,
            format: r#"$scheme $remote_port "$sent_http_content_type" "$upstream_http_server" $upstream_status"#.to_string(),
            ..AccessLogOptions::default()
        },
        &AccessLogRecord {
            scheme: "https".to_string(),
            remote_port: Some(54432),
            sent_response_header_values: BTreeMap::from([(
                "content-type".to_string(),
                "application/json".to_string(),
            )]),
            upstream_response_header_values: BTreeMap::from([(
                "server".to_string(),
                "orders-upstream".to_string(),
            )]),
            upstream_statuses: vec![502, 200],
            ..AccessLogRecord::default()
        },
    )
    .expect("line should render");

    assert_eq!(
        line,
        r#"https 54432 "application/json" "orders-upstream" 502, 200"#
    );
}

#[test]
fn json_access_logs_skip_response_side_render_fields() {
    let line = render_access_log(
        &AccessLogOptions {
            mode: AccessLogMode::Json,
            ..AccessLogOptions::default()
        },
        &AccessLogRecord {
            event: "http_request".to_string(),
            timestamp: "2026-06-16T00:00:00.000Z".to_string(),
            client_ip: "192.0.2.10".to_string(),
            status: Some(200),
            content_type: "application/json".to_string(),
            connection_id: "conn-1".to_string(),
            scheme: "https".to_string(),
            remote_port: Some(54432),
            sent_response_header_values: BTreeMap::from([(
                "content-type".to_string(),
                "application/json".to_string(),
            )]),
            upstream_response_header_values: BTreeMap::from([(
                "server".to_string(),
                "orders-upstream".to_string(),
            )]),
            upstream_statuses: vec![502, 200],
            ..AccessLogRecord::default()
        },
    )
    .expect("line should render");

    let json: serde_json::Value = serde_json::from_str(&line).expect("json should parse");

    assert_eq!(json["event"], "http_request");
    assert_eq!(json["contentType"], "application/json");
    assert_eq!(json["connectionId"], "conn-1");
    assert_eq!(json["status"], 200);
    assert!(json.get("sentResponseHeaderValues").is_none());
    assert!(json.get("upstreamResponseHeaderValues").is_none());
    assert!(json.get("upstreamStatuses").is_none());
    assert!(json.get("scheme").is_none());
    assert!(json.get("remotePort").is_none());
}

#[test]
fn stream_records_render_response_side_http_variables_as_dash() {
    let line = render_access_log(
        &AccessLogOptions {
            mode: AccessLogMode::Text,
            format: r#"$sent_http_content_type $upstream_http_server $upstream_status $scheme $remote_port"#.to_string(),
            ..AccessLogOptions::default()
        },
        &AccessLogRecord {
            event: "tcp_session".to_string(),
            protocol: Cow::Borrowed("TCP"),
            scheme: "https".to_string(),
            remote_port: Some(54432),
            sent_response_header_values: BTreeMap::from([(
                "content-type".to_string(),
                "application/json".to_string(),
            )]),
            upstream_response_header_values: BTreeMap::from([(
                "server".to_string(),
                "orders-upstream".to_string(),
            )]),
            upstream_statuses: vec![502, 200],
            ..AccessLogRecord::default()
        },
    )
    .expect("line should render");

    assert_eq!(line, "- - - - -");
}

#[test]
fn other_stream_signatures_render_response_side_http_variables_as_dash() {
    let cases = [
        ("tls event", "tls_session", ""),
        ("udp event", "udp_datagram", ""),
        ("tls protocol", "opaque_stream", "TLS_PASSTHROUGH"),
        ("udp protocol", "opaque_stream", "UDP"),
    ];

    for (case_name, event, protocol) in cases {
        let line = render_access_log(
            &AccessLogOptions {
                mode: AccessLogMode::Text,
                format: r#"$sent_http_content_type $upstream_http_server $upstream_status $scheme $remote_port"#.to_string(),
                ..AccessLogOptions::default()
            },
            &AccessLogRecord {
                event: event.to_string(),
                protocol: Cow::Borrowed(protocol),
                scheme: "https".to_string(),
                remote_port: Some(54432),
                sent_response_header_values: BTreeMap::from([(
                    "content-type".to_string(),
                    "application/json".to_string(),
                )]),
                upstream_response_header_values: BTreeMap::from([(
                    "server".to_string(),
                    "orders-upstream".to_string(),
                )]),
                upstream_statuses: vec![502, 200],
                ..AccessLogRecord::default()
            },
        )
        .expect("line should render");

        assert_eq!(line, "- - - - -", "{case_name} should force dash fallback");
    }
}
