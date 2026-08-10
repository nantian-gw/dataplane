use std::borrow::Cow;

use form_urlencoded::parse;

use super::*;

pub(crate) fn has_non_backend_http_filter(filters: &[Filter]) -> bool {
    filters.iter().any(|filter| {
        filter.request_redirect.is_some()
            || filter.url_rewrite.is_some()
            || filter.header_modifier.is_some()
            || filter.extension_ref.is_some()
    })
}

pub(crate) fn filters_without_request_mirror(filters: &[Filter]) -> Vec<Filter> {
    filters
        .iter()
        .filter(|filter| filter.request_mirror.is_none())
        .cloned()
        .collect()
}

pub(crate) fn normalize_http_path_match(matcher: &HttpMatch) -> MatchedHttpPath {
    if matcher.path.is_empty() {
        default_http_path_match()
    } else {
        MatchedHttpPath {
            path: matcher.path.clone(),
            path_type: matcher.path_type.clone(),
        }
    }
}

pub(crate) fn default_http_path_match() -> MatchedHttpPath {
    MatchedHttpPath {
        path: "/".to_string(),
        path_type: "PathPrefix".to_string(),
    }
}

#[tracing::instrument]
pub(crate) fn matches_http_rule(matcher: &HttpMatch, request: &RequestMeta) -> bool {
    matches_http_path(matcher, request)
        && matches_method(&matcher.method, &request.method)
        && matches_headers(&matcher.headers, &request.headers)
        && matches_query_params(&matcher.query_params, &request.query_params)
}

#[tracing::instrument]
pub(crate) fn matches_grpc_rule(
    matcher: &GrpcMatch,
    request: &RequestMeta,
    grpc: Option<&GrpcPath<'_>>,
) -> bool {
    if !matches_headers(&matcher.headers, &request.headers) {
        return false;
    }

    let Some(grpc) = grpc else {
        return matcher.service.is_empty() && matcher.method.is_empty();
    };

    matches_grpc_value(
        &matcher.service,
        grpc.service,
        &matcher.match_type,
        matcher.compiled_service_regex.as_deref(),
    ) && matches_grpc_value(
        &matcher.method,
        grpc.method,
        &matcher.match_type,
        matcher.compiled_method_regex.as_deref(),
    )
}

pub(crate) fn matches_stream_rule(
    matcher: &StreamMatch,
    port: u32,
    server_name: Option<&str>,
) -> bool {
    if matcher.port != 0 && matcher.port != port {
        return false;
    }

    if matcher.sni_hostname.is_empty() {
        return true;
    }

    let Some(server_name) = server_name.map(normalize_host_ref) else {
        return false;
    };

    hostname_matches(&matcher.sni_hostname, server_name)
}

pub(crate) fn best_stream_rule_match_with_tls_mode(
    matches: &[StreamMatch],
    port: u32,
    server_name: Option<&str>,
    tls_mode: Option<TlsRouteMode>,
) -> Option<StreamMatchScore> {
    if matches.is_empty() {
        return match tls_mode {
            Some(TlsRouteMode::Terminate) => None,
            _ => Some(StreamMatchScore::default()),
        };
    }

    matches
        .iter()
        .filter(|matcher| tls_mode.is_none_or(|mode| matcher.mode == mode))
        .filter(|matcher| matches_stream_rule(matcher, port, server_name))
        .map(stream_match_score)
        .max()
}

fn stream_match_score(matcher: &StreamMatch) -> StreamMatchScore {
    if matcher.sni_hostname.is_empty() {
        return StreamMatchScore::default();
    }

    let hostname = normalize_host_ref(&matcher.sni_hostname);
    if let Some(suffix) = hostname.strip_prefix("*.") {
        return StreamMatchScore {
            hostname_rank: 1,
            hostname_length: suffix.len(),
        };
    }

    StreamMatchScore {
        hostname_rank: 2,
        hostname_length: hostname.len(),
    }
}

fn matches_http_path(matcher: &HttpMatch, request: &RequestMeta) -> bool {
    if matcher.path.is_empty() {
        return true;
    }

    match matcher.path_type.as_str() {
        "Exact" => request.path == matcher.path,
        "RegularExpression" => regex_matches(
            matcher.compiled_path_regex.as_deref(),
            &matcher.path,
            &request.path,
        ),
        _ => matches_path_prefix(&matcher.path, &request.path),
    }
}

fn matches_path_prefix(prefix: &str, path: &str) -> bool {
    if prefix == "/" {
        return true;
    }
    if path == prefix {
        return true;
    }
    if prefix.ends_with('/') {
        return path.starts_with(prefix);
    }

    path.starts_with(prefix)
        && path
            .as_bytes()
            .get(prefix.len())
            .is_some_and(|item| *item == b'/')
}

fn matches_method(expected: &str, actual: &str) -> bool {
    expected.is_empty() || expected.eq_ignore_ascii_case(actual)
}

fn matches_headers(expected: &[HeaderMatch], actual: &BTreeMap<String, Vec<String>>) -> bool {
    expected.iter().all(|item| {
        let name = lookup_match_name(&item.name);
        let values = actual.get(name.as_ref());
        matches_values(
            &item.value,
            item.match_type.as_str(),
            values,
            item.compiled_regex.as_deref(),
        )
    })
}

fn matches_query_params(expected: &[QueryMatch], actual: &BTreeMap<String, Vec<String>>) -> bool {
    expected.iter().all(|item| {
        let name = lookup_match_name(&item.name);
        let values = actual.get(name.as_ref());
        matches_values(
            &item.value,
            item.match_type.as_str(),
            values,
            item.compiled_regex.as_deref(),
        )
    })
}

fn matches_values(
    expected: &str,
    match_type: &str,
    actual: Option<&Vec<String>>,
    compiled_regex: Option<&Regex>,
) -> bool {
    let Some(values) = actual else {
        return false;
    };

    match match_type {
        "RegularExpression" => values
            .iter()
            .any(|value| regex_matches(compiled_regex, expected, value)),
        _ => values.iter().any(|value| value == expected),
    }
}

pub(crate) fn hostname_matches(pattern: &str, host: &str) -> bool {
    let pattern = normalize_host_ref(pattern);
    if let Some(suffix) = pattern.strip_prefix("*.") {
        host.len() > suffix.len()
            && host.as_bytes()[host.len() - suffix.len() - 1] == b'.'
            && host[host.len() - suffix.len()..].eq_ignore_ascii_case(suffix)
    } else {
        pattern.eq_ignore_ascii_case(host)
    }
}

fn normalize_host(host: String) -> String {
    normalize_host_ref(&host).to_string()
}

pub(crate) fn normalize_host_ref(host: &str) -> &str {
    if let Some(host) = host.strip_prefix('[') {
        return host.split_once(']').map(|(value, _)| value).unwrap_or(host);
    }
    host.split(':').next().unwrap_or(host)
}

fn parse_authority_port(host: &str) -> Option<u32> {
    if let Some(host) = host.strip_prefix('[') {
        return host
            .split_once("]:")
            .and_then(|(_, port)| port.parse::<u32>().ok());
    }

    let (name, port) = host.rsplit_once(':')?;
    if name.contains(':') {
        return None;
    }
    port.parse::<u32>().ok()
}

fn matches_grpc_value(
    expected: &str,
    actual: &str,
    match_type: &str,
    compiled_regex: Option<&Regex>,
) -> bool {
    if expected.is_empty() {
        return true;
    }

    match match_type {
        "RegularExpression" => regex_matches(compiled_regex, expected, actual),
        _ => expected == actual,
    }
}

fn regex_matches(compiled_regex: Option<&Regex>, pattern: &str, candidate: &str) -> bool {
    compiled_regex
        .map(|regex| regex.is_match(candidate))
        .unwrap_or_else(|| {
            Regex::new(pattern)
                .map(|regex| regex.is_match(candidate))
                .unwrap_or(false)
        })
}

fn compile_regex(pattern: &str, enabled: bool) -> Option<Arc<Regex>> {
    enabled
        .then(|| Regex::new(pattern).ok().map(Arc::new))
        .flatten()
}

impl HttpMatch {
    pub(crate) fn compile_runtime_state(&mut self) {
        self.compiled_path_regex = compile_regex(&self.path, self.path_type == "RegularExpression");
        for header in &mut self.headers {
            header.compile_runtime_state();
        }
        for query in &mut self.query_params {
            query.compile_runtime_state();
        }
    }
}

impl GrpcMatch {
    pub(crate) fn compile_runtime_state(&mut self) {
        let regex_enabled = self.match_type == "RegularExpression";
        self.compiled_service_regex =
            compile_regex(&self.service, regex_enabled && !self.service.is_empty());
        self.compiled_method_regex =
            compile_regex(&self.method, regex_enabled && !self.method.is_empty());
        for header in &mut self.headers {
            header.compile_runtime_state();
        }
    }
}

impl HeaderMatch {
    fn compile_runtime_state(&mut self) {
        normalize_match_name(&mut self.name);
        self.compiled_regex = compile_regex(&self.value, self.match_type == "RegularExpression");
    }
}

impl QueryMatch {
    fn compile_runtime_state(&mut self) {
        normalize_match_name(&mut self.name);
        self.compiled_regex = compile_regex(&self.value, self.match_type == "RegularExpression");
    }
}

fn lookup_match_name(name: &str) -> Cow<'_, str> {
    if has_ascii_uppercase(name) {
        Cow::Owned(name.to_ascii_lowercase())
    } else {
        Cow::Borrowed(name)
    }
}

fn normalize_match_name(name: &mut str) {
    if has_ascii_uppercase(name) {
        name.make_ascii_lowercase();
    }
}

fn has_ascii_uppercase(value: &str) -> bool {
    value.bytes().any(|byte| byte.is_ascii_uppercase())
}

pub(crate) fn is_grpc_request(request: &RequestMeta) -> bool {
    request.headers.get("content-type").is_some_and(|values| {
        values
            .iter()
            .any(|value| value.starts_with("application/grpc"))
    })
}

#[derive(Debug)]
pub(crate) struct GrpcPath<'a> {
    pub(crate) service: &'a str,
    pub(crate) method: &'a str,
}

pub(crate) fn parse_grpc_path(path: &str) -> Option<GrpcPath<'_>> {
    let path = path.strip_prefix('/')?;
    let (service, method) = path.split_once('/')?;
    if service.is_empty() || method.is_empty() {
        return None;
    }

    Some(GrpcPath { service, method })
}

impl RequestMeta {
    pub fn new(
        host: Option<String>,
        path_and_query: &str,
        method: &str,
        headers: BTreeMap<String, Vec<String>>,
    ) -> Self {
        Self::with_port(host, 0, path_and_query, method, headers)
    }

    pub fn with_port(
        host: Option<String>,
        port: u32,
        path_and_query: &str,
        method: &str,
        headers: BTreeMap<String, Vec<String>>,
    ) -> Self {
        let (path, query_params) = split_path_and_query(path_and_query);
        let resolved_port = if port != 0 {
            port
        } else {
            host.as_deref()
                .and_then(parse_authority_port)
                .unwrap_or_default()
        };
        let normalized_host = host.map(normalize_host);
        Self {
            host: normalized_host,
            port: resolved_port,
            path,
            method: method.to_ascii_uppercase(),
            source_ip: None,
            headers,
            query_params,
        }
    }
}

fn split_path_and_query(raw: &str) -> (String, BTreeMap<String, Vec<String>>) {
    let mut parts = raw.splitn(2, '?');
    let path = parts.next().unwrap_or("/").to_string();
    let query = parts.next().unwrap_or("");
    let mut query_params: BTreeMap<String, Vec<String>> = BTreeMap::new();

    for (name, value) in parse(query.as_bytes()) {
        query_params
            .entry(name.into_owned().to_ascii_lowercase())
            .or_default()
            .push(value.into_owned());
    }

    (path, query_params)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hostname_matches_is_case_insensitive() {
        // Exact match
        assert!(hostname_matches("example.com", "example.com"));
        assert!(hostname_matches("example.com", "Example.com"));
        assert!(hostname_matches("Example.com", "example.com"));
        assert!(hostname_matches("EXAMPLE.COM", "example.com"));

        // Wildcard match
        assert!(hostname_matches("*.example.com", "foo.example.com"));
        assert!(hostname_matches("*.example.com", "Foo.Example.com"));
        assert!(hostname_matches("*.Example.com", "foo.example.com"));

        // Wildcard should NOT match the base domain
        assert!(!hostname_matches("*.example.com", "example.com"));
        assert!(!hostname_matches("*.example.com", "Example.com"));

        // Non-matching
        assert!(!hostname_matches("example.com", "other.com"));
        assert!(!hostname_matches("*.example.com", "foo.other.com"));
    }
}
