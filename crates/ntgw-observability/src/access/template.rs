use std::fmt::Write as _;

use super::AccessLogRecord;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum AccessLogPlaceholder {
    Timestamp,
    StartTime,
    SnapshotVersion,
    Event,
    Listener,
    ListenerRuntimeID,
    Protocol,
    ClientIP,
    Host,
    Method,
    Path,
    RequestID,
    RouteNamespace,
    RouteName,
    RouteKind,
    RouteRuntimeID,
    RuleRuntimeID,
    Backend,
    BackendRuntimeID,
    EndpointRuntimeID,
    Status,
    LatencyMs,
    BytesSent,
    BytesReceived,
    RetryAttempts,
    ResponseFlags,
    Request,
    HttpVersion,
    QueryString,
    Referer,
    UserAgent,
    XForwardedFor,
    UpstreamAddr,
    UpstreamConnectTimeMs,
    ContentType,
    ConnectionId,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum AccessLogTemplatePart {
    Literal(String),
    Placeholder(AccessLogPlaceholder),
}

pub(super) fn render_access_log_template(template: &str, record: &AccessLogRecord) -> String {
    let parts = parse_access_log_template(template);
    let mut line = String::with_capacity(template.len().saturating_add(64));
    for part in parts {
        match part {
            AccessLogTemplatePart::Literal(value) => line.push_str(&value),
            AccessLogTemplatePart::Placeholder(placeholder) => {
                write_access_log_placeholder(&mut line, placeholder, record)
            }
        }
    }
    line
}

pub(super) fn parse_access_log_template(template: &str) -> Vec<AccessLogTemplatePart> {
    if template.is_empty() {
        return vec![AccessLogTemplatePart::Literal(String::new())];
    }

    let mut parts = Vec::new();
    let mut literal_start = 0;
    let mut scan = 0;
    let bytes = template.as_bytes();

    while scan < bytes.len() {
        if bytes[scan] != b'%' {
            scan += 1;
            continue;
        }

        let Some(close_offset) = template[scan + 1..].find('%') else {
            break;
        };
        let close = scan + 1 + close_offset;
        let token = &template[scan..=close];

        if let Some(placeholder) = parse_access_log_placeholder(token) {
            if literal_start < scan {
                parts.push(AccessLogTemplatePart::Literal(
                    template[literal_start..scan].to_string(),
                ));
            }
            parts.push(AccessLogTemplatePart::Placeholder(placeholder));
            literal_start = close + 1;
        }

        scan = close + 1;
    }

    if literal_start < template.len() {
        parts.push(AccessLogTemplatePart::Literal(
            template[literal_start..].to_string(),
        ));
    }

    if parts.is_empty() {
        parts.push(AccessLogTemplatePart::Literal(template.to_string()));
    }

    parts
}

fn parse_access_log_placeholder(token: &str) -> Option<AccessLogPlaceholder> {
    match token {
        "%TIMESTAMP%" => Some(AccessLogPlaceholder::Timestamp),
        "%START_TIME%" => Some(AccessLogPlaceholder::StartTime),
        "%SNAPSHOT_VERSION%" => Some(AccessLogPlaceholder::SnapshotVersion),
        "%EVENT%" => Some(AccessLogPlaceholder::Event),
        "%LISTENER%" => Some(AccessLogPlaceholder::Listener),
        "%LISTENER_RUNTIME_ID%" => Some(AccessLogPlaceholder::ListenerRuntimeID),
        "%PROTOCOL%" => Some(AccessLogPlaceholder::Protocol),
        "%CLIENT_IP%" => Some(AccessLogPlaceholder::ClientIP),
        "%HOST%" => Some(AccessLogPlaceholder::Host),
        "%METHOD%" => Some(AccessLogPlaceholder::Method),
        "%PATH%" => Some(AccessLogPlaceholder::Path),
        "%REQUEST_ID%" => Some(AccessLogPlaceholder::RequestID),
        "%ROUTE_NAMESPACE%" => Some(AccessLogPlaceholder::RouteNamespace),
        "%ROUTE_NAME%" => Some(AccessLogPlaceholder::RouteName),
        "%ROUTE_KIND%" => Some(AccessLogPlaceholder::RouteKind),
        "%ROUTE_RUNTIME_ID%" => Some(AccessLogPlaceholder::RouteRuntimeID),
        "%RULE_RUNTIME_ID%" => Some(AccessLogPlaceholder::RuleRuntimeID),
        "%BACKEND%" => Some(AccessLogPlaceholder::Backend),
        "%BACKEND_RUNTIME_ID%" => Some(AccessLogPlaceholder::BackendRuntimeID),
        "%ENDPOINT_RUNTIME_ID%" => Some(AccessLogPlaceholder::EndpointRuntimeID),
        "%STATUS%" => Some(AccessLogPlaceholder::Status),
        "%LATENCY_MS%" => Some(AccessLogPlaceholder::LatencyMs),
        "%BYTES_SENT%" => Some(AccessLogPlaceholder::BytesSent),
        "%BYTES_RECEIVED%" => Some(AccessLogPlaceholder::BytesReceived),
        "%RETRY_ATTEMPTS%" => Some(AccessLogPlaceholder::RetryAttempts),
        "%RESPONSE_FLAGS%" => Some(AccessLogPlaceholder::ResponseFlags),
        "%REQUEST%" => Some(AccessLogPlaceholder::Request),
        "%HTTP_VERSION%" => Some(AccessLogPlaceholder::HttpVersion),
        "%QUERY_STRING%" => Some(AccessLogPlaceholder::QueryString),
        "%REFERER%" => Some(AccessLogPlaceholder::Referer),
        "%USER_AGENT%" => Some(AccessLogPlaceholder::UserAgent),
        "%X_FORWARDED_FOR%" => Some(AccessLogPlaceholder::XForwardedFor),
        "%UPSTREAM_ADDR%" => Some(AccessLogPlaceholder::UpstreamAddr),
        "%UPSTREAM_CONNECT_TIME_MS%" => Some(AccessLogPlaceholder::UpstreamConnectTimeMs),
        "%CONTENT_TYPE%" => Some(AccessLogPlaceholder::ContentType),
        "%CONNECTION_ID%" => Some(AccessLogPlaceholder::ConnectionId),
        _ => None,
    }
}

fn write_access_log_placeholder(
    out: &mut String,
    placeholder: AccessLogPlaceholder,
    record: &AccessLogRecord,
) {
    match placeholder {
        AccessLogPlaceholder::Timestamp => push_value_or_dash(out, &record.timestamp),
        AccessLogPlaceholder::StartTime => {
            let _ = write!(out, "{}", record.start_time_unix_ms);
        }
        AccessLogPlaceholder::SnapshotVersion => push_value_or_dash(out, &record.snapshot_version),
        AccessLogPlaceholder::Event => push_value_or_dash(out, &record.event),
        AccessLogPlaceholder::Listener => push_value_or_dash(out, &record.listener),
        AccessLogPlaceholder::ListenerRuntimeID => {
            push_optional_value_or_dash(out, record.listener_runtime_id.as_deref())
        }
        AccessLogPlaceholder::Protocol => push_value_or_dash(out, &record.protocol),
        AccessLogPlaceholder::ClientIP => push_value_or_dash(out, &record.client_ip),
        AccessLogPlaceholder::Host => push_value_or_dash(out, &record.host),
        AccessLogPlaceholder::Method => push_value_or_dash(out, &record.method),
        AccessLogPlaceholder::Path => push_value_or_dash(out, &record.path),
        AccessLogPlaceholder::RequestID => push_value_or_dash(out, &record.request_id),
        AccessLogPlaceholder::RouteNamespace => push_value_or_dash(out, &record.route_namespace),
        AccessLogPlaceholder::RouteName => push_value_or_dash(out, &record.route_name),
        AccessLogPlaceholder::RouteKind => push_value_or_dash(out, &record.route_kind),
        AccessLogPlaceholder::RouteRuntimeID => {
            push_optional_value_or_dash(out, record.route_runtime_id.as_deref())
        }
        AccessLogPlaceholder::RuleRuntimeID => {
            push_optional_value_or_dash(out, record.rule_runtime_id.as_deref())
        }
        AccessLogPlaceholder::Backend => push_value_or_dash(out, &record.backend),
        AccessLogPlaceholder::BackendRuntimeID => {
            push_optional_value_or_dash(out, record.backend_runtime_id.as_deref())
        }
        AccessLogPlaceholder::EndpointRuntimeID => {
            push_optional_value_or_dash(out, record.endpoint_runtime_id.as_deref())
        }
        AccessLogPlaceholder::Status => match record.status {
            Some(value) => {
                let _ = write!(out, "{value}");
            }
            None => out.push('-'),
        },
        AccessLogPlaceholder::LatencyMs => {
            let _ = write!(out, "{}", record.latency_ms);
        }
        AccessLogPlaceholder::BytesSent => {
            let _ = write!(out, "{}", record.bytes_sent);
        }
        AccessLogPlaceholder::BytesReceived => {
            let _ = write!(out, "{}", record.bytes_received);
        }
        AccessLogPlaceholder::RetryAttempts => {
            let _ = write!(out, "{}", record.retry_attempts);
        }
        AccessLogPlaceholder::ResponseFlags => push_value_or_dash(out, &record.response_flags),
        AccessLogPlaceholder::Request => push_value_or_dash(out, &record.request),
        AccessLogPlaceholder::HttpVersion => push_value_or_dash(out, &record.http_version),
        AccessLogPlaceholder::QueryString => push_value_or_dash(out, &record.query_string),
        AccessLogPlaceholder::Referer => push_value_or_dash(out, &record.referer),
        AccessLogPlaceholder::UserAgent => push_value_or_dash(out, &record.user_agent),
        AccessLogPlaceholder::XForwardedFor => push_value_or_dash(out, &record.x_forwarded_for),
        AccessLogPlaceholder::UpstreamAddr => push_value_or_dash(out, &record.upstream_addr),
        AccessLogPlaceholder::UpstreamConnectTimeMs => {
            let _ = write!(out, "{}", record.upstream_connect_time_ms);
        }
        AccessLogPlaceholder::ContentType => push_value_or_dash(out, &record.content_type),
        AccessLogPlaceholder::ConnectionId => push_value_or_dash(out, &record.connection_id),
    }
}

fn push_value_or_dash(out: &mut String, value: &str) {
    if value.trim().is_empty() {
        out.push('-');
    } else {
        out.push_str(value);
    }
}

fn push_optional_value_or_dash(out: &mut String, value: Option<&str>) {
    match value {
        Some(value) => push_value_or_dash(out, value),
        None => out.push('-'),
    }
}
