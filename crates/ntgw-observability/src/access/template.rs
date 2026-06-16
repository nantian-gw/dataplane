use std::{collections::BTreeSet, fmt::Write as _};

use super::{AccessLogRecord, AccessLogTemplateRequirements};

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

#[cfg(test)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum AccessLogTemplatePart {
    Literal(String),
    Placeholder(AccessLogPlaceholder),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum AccessLogVariable {
    RemoteAddr,
    Host,
    RequestMethod,
    RequestUri,
    Uri,
    Args,
    Request,
    Status,
    RequestTime,
    BytesSent,
    RequestId,
    UpstreamAddr,
    UpstreamConnectTime,
    ServerProtocol,
    TimeIso8601,
    NtgwEvent,
    NtgwProtocol,
    NtgwListener,
    NtgwRouteNamespace,
    NtgwRouteName,
    NtgwRouteKind,
    NtgwBackend,
    NtgwSnapshotVersion,
    NtgwRetryAttempts,
    NtgwResponseFlags,
    RequestHeader(String),
    SentResponseHeader(String),
    UpstreamResponseHeader(String),
    UpstreamStatus,
    Scheme,
    RemotePort,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum CompiledAccessLogTemplatePart {
    Literal(String),
    LegacyPlaceholder(AccessLogPlaceholder),
    Variable(AccessLogVariable),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct CompiledAccessLogTemplate {
    pub parts: Vec<CompiledAccessLogTemplatePart>,
    pub requirements: AccessLogTemplateRequirements,
    pub unknown_tokens: Vec<String>,
}

#[cfg(test)]
pub(super) fn render_access_log_template(template: &str, record: &AccessLogRecord) -> String {
    let compiled = compile_access_log_template(template);
    render_compiled_access_log_template(&compiled, record)
}

#[cfg(test)]
pub(super) fn parse_access_log_template(template: &str) -> Vec<AccessLogTemplatePart> {
    compile_access_log_template(template)
        .parts
        .into_iter()
        .map(|part| match part {
            CompiledAccessLogTemplatePart::Literal(value) => AccessLogTemplatePart::Literal(value),
            CompiledAccessLogTemplatePart::LegacyPlaceholder(placeholder) => {
                AccessLogTemplatePart::Placeholder(placeholder)
            }
            CompiledAccessLogTemplatePart::Variable(variable) => {
                AccessLogTemplatePart::Literal(variable_token(&variable))
            }
        })
        .collect()
}

pub(super) fn compile_access_log_template(template: &str) -> CompiledAccessLogTemplate {
    if template.is_empty() {
        return CompiledAccessLogTemplate {
            parts: vec![CompiledAccessLogTemplatePart::Literal(String::new())],
            requirements: AccessLogTemplateRequirements::default(),
            unknown_tokens: Vec::new(),
        };
    }

    let chars: Vec<char> = template.chars().collect();
    let mut parts = Vec::new();
    let mut requirements = AccessLogTemplateRequirements::default();
    let mut unknown_tokens = Vec::new();
    let mut literal = String::new();
    let mut index = 0;

    while index < chars.len() {
        if chars[index] == '%'
            && let Some(close_offset) = chars[index + 1..].iter().position(|ch| *ch == '%')
        {
            let close = index + close_offset + 1;
            let token: String = chars[index..=close].iter().collect();
            if let Some(placeholder) = parse_access_log_placeholder(&token) {
                flush_literal(&mut literal, &mut parts);
                parts.push(CompiledAccessLogTemplatePart::LegacyPlaceholder(
                    placeholder,
                ));
                index = close + 1;
                continue;
            }

            push_unknown_token(&mut unknown_tokens, &token);
            literal.push_str(&token);
            index = close + 1;
            continue;
        }

        if chars[index] == '$'
            && let Some((token, consumed)) = parse_nginx_style_variable(&chars[index..])
        {
            if let Some((variable, variable_requirements)) = parse_access_log_variable(&token) {
                flush_literal(&mut literal, &mut parts);
                requirements.merge(variable_requirements);
                parts.push(CompiledAccessLogTemplatePart::Variable(variable));
                index += consumed;
                continue;
            }

            push_unknown_token(&mut unknown_tokens, &token);
            literal.push_str(&token);
            index += consumed;
            continue;
        }

        literal.push(chars[index]);
        index += 1;
    }

    flush_literal(&mut literal, &mut parts);

    if parts.is_empty() {
        parts.push(CompiledAccessLogTemplatePart::Literal(String::new()));
    }

    CompiledAccessLogTemplate {
        parts,
        requirements,
        unknown_tokens,
    }
}

pub(super) fn render_compiled_access_log_template(
    template: &CompiledAccessLogTemplate,
    record: &AccessLogRecord,
) -> String {
    let mut line = String::new();
    for part in &template.parts {
        match part {
            CompiledAccessLogTemplatePart::Literal(value) => line.push_str(value),
            CompiledAccessLogTemplatePart::LegacyPlaceholder(placeholder) => {
                write_access_log_placeholder(&mut line, *placeholder, record);
            }
            CompiledAccessLogTemplatePart::Variable(variable) => {
                render_variable(&mut line, variable, record);
            }
        }
    }
    line
}

impl AccessLogTemplateRequirements {
    pub(super) fn merge(&mut self, other: Self) {
        self.uses_request_line |= other.uses_request_line;
        self.uses_query_string |= other.uses_query_string;
        self.uses_http_only_variables |= other.uses_http_only_variables;
        self.request_headers.extend(other.request_headers);
        self.sent_response_headers
            .extend(other.sent_response_headers);
        self.upstream_response_headers
            .extend(other.upstream_response_headers);
        self.needs_upstream_status |= other.needs_upstream_status;
        self.needs_scheme |= other.needs_scheme;
        self.needs_remote_port |= other.needs_remote_port;
    }
}

enum ResponseHeaderDomain {
    Sent,
    Upstream,
}

fn parse_nginx_style_variable(chars: &[char]) -> Option<(String, usize)> {
    if chars.first() != Some(&'$') {
        return None;
    }

    if chars.get(1) == Some(&'{') {
        let close = chars[2..]
            .iter()
            .position(|ch| *ch == '}')
            .map(|offset| offset + 2)?;
        return Some((chars[..=close].iter().collect(), close + 1));
    }

    let first = chars.get(1).copied()?;
    if !first.is_ascii_alphabetic() && first != '_' {
        return None;
    }

    let mut consumed = 2;
    while consumed < chars.len() {
        let ch = chars[consumed];
        if ch.is_ascii_alphanumeric() || ch == '_' {
            consumed += 1;
        } else {
            break;
        }
    }

    Some((chars[..consumed].iter().collect(), consumed))
}

fn parse_access_log_variable(
    token: &str,
) -> Option<(AccessLogVariable, AccessLogTemplateRequirements)> {
    let normalized = token
        .strip_prefix("${")
        .and_then(|value| value.strip_suffix('}'))
        .unwrap_or_else(|| token.trim_start_matches('$'))
        .trim();

    if normalized.is_empty() {
        return None;
    }

    let normalized = normalized.to_ascii_lowercase();
    let requirements = |uses_request_line, uses_query_string, uses_http_only_variables| {
        AccessLogTemplateRequirements {
            uses_request_line,
            uses_query_string,
            uses_http_only_variables,
            request_headers: BTreeSet::new(),
            sent_response_headers: BTreeSet::new(),
            upstream_response_headers: BTreeSet::new(),
            needs_upstream_status: false,
            needs_scheme: false,
            needs_remote_port: false,
        }
    };

    let request_header = |name: String| {
        let mut reqs = AccessLogTemplateRequirements {
            uses_http_only_variables: true,
            ..AccessLogTemplateRequirements::default()
        };
        reqs.request_headers.insert(name.clone());
        (AccessLogVariable::RequestHeader(name), reqs)
    };

    let response_header = |name: String, domain: ResponseHeaderDomain| {
        let mut reqs = AccessLogTemplateRequirements {
            uses_http_only_variables: true,
            ..AccessLogTemplateRequirements::default()
        };
        match domain {
            ResponseHeaderDomain::Sent => {
                reqs.sent_response_headers.insert(name.clone());
                (AccessLogVariable::SentResponseHeader(name), reqs)
            }
            ResponseHeaderDomain::Upstream => {
                reqs.upstream_response_headers.insert(name.clone());
                (AccessLogVariable::UpstreamResponseHeader(name), reqs)
            }
        }
    };

    match normalized.as_str() {
        "remote_addr" => Some((
            AccessLogVariable::RemoteAddr,
            requirements(false, false, true),
        )),
        "host" => Some((AccessLogVariable::Host, requirements(false, false, true))),
        "request_method" => Some((
            AccessLogVariable::RequestMethod,
            requirements(false, false, true),
        )),
        "request_uri" => Some((
            AccessLogVariable::RequestUri,
            requirements(false, true, true),
        )),
        "uri" => Some((AccessLogVariable::Uri, requirements(false, false, true))),
        "args" => Some((AccessLogVariable::Args, requirements(false, true, true))),
        "request" => Some((AccessLogVariable::Request, requirements(true, true, true))),
        "status" => Some((AccessLogVariable::Status, requirements(false, false, false))),
        "request_time" => Some((
            AccessLogVariable::RequestTime,
            requirements(false, false, false),
        )),
        "bytes_sent" => Some((
            AccessLogVariable::BytesSent,
            requirements(false, false, false),
        )),
        "request_id" => Some((
            AccessLogVariable::RequestId,
            requirements(false, false, true),
        )),
        "upstream_addr" => Some((
            AccessLogVariable::UpstreamAddr,
            requirements(false, false, true),
        )),
        "upstream_connect_time" => Some((
            AccessLogVariable::UpstreamConnectTime,
            requirements(false, false, false),
        )),
        "server_protocol" => Some((
            AccessLogVariable::ServerProtocol,
            requirements(false, false, true),
        )),
        "time_iso8601" => Some((
            AccessLogVariable::TimeIso8601,
            requirements(false, false, false),
        )),
        "ntgw_event" => Some((
            AccessLogVariable::NtgwEvent,
            requirements(false, false, false),
        )),
        "ntgw_protocol" => Some((
            AccessLogVariable::NtgwProtocol,
            requirements(false, false, false),
        )),
        "ntgw_listener" => Some((
            AccessLogVariable::NtgwListener,
            requirements(false, false, false),
        )),
        "ntgw_route_namespace" => Some((
            AccessLogVariable::NtgwRouteNamespace,
            requirements(false, false, false),
        )),
        "ntgw_route_name" => Some((
            AccessLogVariable::NtgwRouteName,
            requirements(false, false, false),
        )),
        "ntgw_route_kind" => Some((
            AccessLogVariable::NtgwRouteKind,
            requirements(false, false, false),
        )),
        "ntgw_backend" => Some((
            AccessLogVariable::NtgwBackend,
            requirements(false, false, false),
        )),
        "ntgw_snapshot_version" => Some((
            AccessLogVariable::NtgwSnapshotVersion,
            requirements(false, false, false),
        )),
        "ntgw_retry_attempts" => Some((
            AccessLogVariable::NtgwRetryAttempts,
            requirements(false, false, false),
        )),
        "ntgw_response_flags" => Some((
            AccessLogVariable::NtgwResponseFlags,
            requirements(false, false, false),
        )),
        _ if normalized.starts_with("http_") => Some(request_header(
            normalized.trim_start_matches("http_").replace('_', "-"),
        )),
        _ if normalized.starts_with("sent_http_") => Some(response_header(
            normalized
                .trim_start_matches("sent_http_")
                .replace('_', "-"),
            ResponseHeaderDomain::Sent,
        )),
        _ if normalized.starts_with("upstream_http_") => Some(response_header(
            normalized
                .trim_start_matches("upstream_http_")
                .replace('_', "-"),
            ResponseHeaderDomain::Upstream,
        )),
        "upstream_status" => Some((
            AccessLogVariable::UpstreamStatus,
            AccessLogTemplateRequirements {
                needs_upstream_status: true,
                ..requirements(false, false, true)
            },
        )),
        "scheme" => Some((
            AccessLogVariable::Scheme,
            AccessLogTemplateRequirements {
                needs_scheme: true,
                ..requirements(false, false, true)
            },
        )),
        "remote_port" => Some((
            AccessLogVariable::RemotePort,
            AccessLogTemplateRequirements {
                needs_remote_port: true,
                ..requirements(false, false, true)
            },
        )),
        _ => None,
    }
}

fn request_uri(record: &AccessLogRecord) -> String {
    if record.path.is_empty() {
        return String::new();
    }
    if record.query_string.is_empty() {
        record.path.clone()
    } else {
        format!("{}?{}", record.path, record.query_string)
    }
}

fn seconds_with_millis(value_ms: u128) -> String {
    format!("{}.{:03}", value_ms / 1_000, value_ms % 1_000)
}

fn is_stream_access_log_record(record: &AccessLogRecord) -> bool {
    matches!(
        record.event.as_str(),
        "tcp_session" | "tls_session" | "udp_datagram"
    ) || matches!(record.protocol.as_ref(), "TCP" | "TLS_PASSTHROUGH" | "UDP")
}

fn render_variable(out: &mut String, variable: &AccessLogVariable, record: &AccessLogRecord) {
    let is_stream_record = is_stream_access_log_record(record);
    match variable {
        AccessLogVariable::RemoteAddr => push_value_or_dash(out, &record.client_ip),
        AccessLogVariable::Host => push_value_or_dash(out, &record.host),
        AccessLogVariable::RequestMethod => push_value_or_dash(out, &record.method),
        AccessLogVariable::RequestUri => {
            let value = request_uri(record);
            push_value_or_dash(out, &value);
        }
        AccessLogVariable::Uri => push_value_or_dash(out, &record.path),
        AccessLogVariable::Args => push_value_or_dash(out, &record.query_string),
        AccessLogVariable::Request => push_value_or_dash(out, &record.request),
        AccessLogVariable::Status => match record.status {
            Some(status) => {
                let _ = write!(out, "{status}");
            }
            None => out.push('-'),
        },
        AccessLogVariable::RequestTime => out.push_str(&seconds_with_millis(record.latency_ms)),
        AccessLogVariable::BytesSent => {
            let _ = write!(out, "{}", record.bytes_sent);
        }
        AccessLogVariable::RequestId => push_value_or_dash(out, &record.request_id),
        AccessLogVariable::UpstreamAddr => push_value_or_dash(out, &record.upstream_addr),
        AccessLogVariable::UpstreamConnectTime => {
            out.push_str(&seconds_with_millis(record.upstream_connect_time_ms));
        }
        AccessLogVariable::ServerProtocol => push_value_or_dash(out, &record.http_version),
        AccessLogVariable::TimeIso8601 => push_value_or_dash(out, &record.timestamp),
        AccessLogVariable::NtgwEvent => push_value_or_dash(out, &record.event),
        AccessLogVariable::NtgwProtocol => push_value_or_dash(out, &record.protocol),
        AccessLogVariable::NtgwListener => push_value_or_dash(out, &record.listener),
        AccessLogVariable::NtgwRouteNamespace => push_value_or_dash(out, &record.route_namespace),
        AccessLogVariable::NtgwRouteName => push_value_or_dash(out, &record.route_name),
        AccessLogVariable::NtgwRouteKind => push_value_or_dash(out, &record.route_kind),
        AccessLogVariable::NtgwBackend => push_value_or_dash(out, &record.backend),
        AccessLogVariable::NtgwSnapshotVersion => push_value_or_dash(out, &record.snapshot_version),
        AccessLogVariable::NtgwRetryAttempts => {
            let _ = write!(out, "{}", record.retry_attempts);
        }
        AccessLogVariable::NtgwResponseFlags => push_value_or_dash(out, &record.response_flags),
        AccessLogVariable::RequestHeader(name) => match record.request_header_values.get(name) {
            Some(value) => push_value_or_dash(out, value),
            None => out.push('-'),
        },
        AccessLogVariable::SentResponseHeader(name) => {
            if is_stream_record {
                out.push('-');
                return;
            }
            match record.sent_response_header_values.get(name) {
                Some(value) => push_value_or_dash(out, value),
                None => out.push('-'),
            }
        }
        AccessLogVariable::UpstreamResponseHeader(name) => {
            if is_stream_record {
                out.push('-');
                return;
            }
            match record.upstream_response_header_values.get(name) {
                Some(value) => push_value_or_dash(out, value),
                None => out.push('-'),
            }
        }
        AccessLogVariable::UpstreamStatus => {
            if is_stream_record {
                out.push('-');
                return;
            }
            if record.upstream_statuses.is_empty() {
                out.push('-');
            } else {
                for (index, status) in record.upstream_statuses.iter().enumerate() {
                    if index > 0 {
                        out.push_str(", ");
                    }
                    let _ = write!(out, "{status}");
                }
            }
        }
        AccessLogVariable::Scheme => {
            if is_stream_record {
                out.push('-');
            } else {
                push_value_or_dash(out, &record.scheme);
            }
        }
        AccessLogVariable::RemotePort => match record.remote_port {
            Some(port) if !is_stream_record => {
                let _ = write!(out, "{port}");
            }
            _ => out.push('-'),
        },
    }
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

fn flush_literal(literal: &mut String, parts: &mut Vec<CompiledAccessLogTemplatePart>) {
    if !literal.is_empty() {
        parts.push(CompiledAccessLogTemplatePart::Literal(std::mem::take(
            literal,
        )));
    }
}

fn push_unknown_token(unknown_tokens: &mut Vec<String>, token: &str) {
    if unknown_tokens.iter().all(|existing| existing != token) {
        unknown_tokens.push(token.to_string());
    }
}

#[cfg(test)]
fn variable_token(variable: &AccessLogVariable) -> String {
    match variable {
        AccessLogVariable::RemoteAddr => "$remote_addr".to_string(),
        AccessLogVariable::Host => "$host".to_string(),
        AccessLogVariable::RequestMethod => "$request_method".to_string(),
        AccessLogVariable::RequestUri => "$request_uri".to_string(),
        AccessLogVariable::Uri => "$uri".to_string(),
        AccessLogVariable::Args => "$args".to_string(),
        AccessLogVariable::Request => "$request".to_string(),
        AccessLogVariable::Status => "$status".to_string(),
        AccessLogVariable::RequestTime => "$request_time".to_string(),
        AccessLogVariable::BytesSent => "$bytes_sent".to_string(),
        AccessLogVariable::RequestId => "$request_id".to_string(),
        AccessLogVariable::UpstreamAddr => "$upstream_addr".to_string(),
        AccessLogVariable::UpstreamConnectTime => "$upstream_connect_time".to_string(),
        AccessLogVariable::ServerProtocol => "$server_protocol".to_string(),
        AccessLogVariable::TimeIso8601 => "$time_iso8601".to_string(),
        AccessLogVariable::NtgwEvent => "$ntgw_event".to_string(),
        AccessLogVariable::NtgwProtocol => "$ntgw_protocol".to_string(),
        AccessLogVariable::NtgwListener => "$ntgw_listener".to_string(),
        AccessLogVariable::NtgwRouteNamespace => "$ntgw_route_namespace".to_string(),
        AccessLogVariable::NtgwRouteName => "$ntgw_route_name".to_string(),
        AccessLogVariable::NtgwRouteKind => "$ntgw_route_kind".to_string(),
        AccessLogVariable::NtgwBackend => "$ntgw_backend".to_string(),
        AccessLogVariable::NtgwSnapshotVersion => "$ntgw_snapshot_version".to_string(),
        AccessLogVariable::NtgwRetryAttempts => "$ntgw_retry_attempts".to_string(),
        AccessLogVariable::NtgwResponseFlags => "$ntgw_response_flags".to_string(),
        AccessLogVariable::RequestHeader(name) => {
            format!("$http_{}", name.replace('-', "_"))
        }
        AccessLogVariable::SentResponseHeader(name) => {
            format!("$sent_http_{}", name.replace('-', "_"))
        }
        AccessLogVariable::UpstreamResponseHeader(name) => {
            format!("$upstream_http_{}", name.replace('-', "_"))
        }
        AccessLogVariable::UpstreamStatus => "$upstream_status".to_string(),
        AccessLogVariable::Scheme => "$scheme".to_string(),
        AccessLogVariable::RemotePort => "$remote_port".to_string(),
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
