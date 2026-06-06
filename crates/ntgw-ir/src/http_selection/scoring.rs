use super::*;

#[derive(Clone, Copy, Debug, Default, Eq, Ord, PartialEq, PartialOrd)]
pub(super) struct HostnameScore {
    pub(super) rank: u8,
    pub(super) length: usize,
}

#[derive(Clone, Copy, Debug, Default, Eq, Ord, PartialEq, PartialOrd)]
pub(super) struct HttpRuleScore {
    pub(super) path_rank: u8,
    pub(super) path_length: usize,
    pub(super) method_specified: bool,
    pub(super) header_count: usize,
    pub(super) query_count: usize,
}

#[derive(Clone, Copy, Debug, Default, Eq, Ord, PartialEq, PartialOrd)]
pub(super) struct GrpcRuleScore {
    pub(super) service_specified: bool,
    pub(super) method_specified: bool,
    pub(super) header_count: usize,
}

#[derive(Clone, Copy, Debug, Default, Eq, Ord, PartialEq, PartialOrd)]
pub(super) struct HttpCandidateScore {
    pub(super) listener_host: HostnameScore,
    pub(super) route_host: HostnameScore,
    pub(super) rule: HttpRuleScore,
}

#[derive(Clone, Copy, Debug, Default, Eq, Ord, PartialEq, PartialOrd)]
pub(super) struct GrpcCandidateScore {
    pub(super) listener_host: HostnameScore,
    pub(super) route_host: HostnameScore,
    pub(super) rule: GrpcRuleScore,
}

pub(super) fn best_http_rule_match(
    rule: &HttpRule,
    request: &RequestMeta,
) -> Option<(MatchedHttpPath, HttpRuleScore)> {
    if rule.matches.is_empty() {
        return Some((default_http_path_match(), HttpRuleScore::default()));
    }

    rule.matches
        .iter()
        .filter(|matcher| matches_http_rule(matcher, request))
        .map(|matcher| {
            let matched_path = normalize_http_path_match(matcher);
            let score = HttpRuleScore {
                path_rank: http_path_rank(&matched_path.path_type),
                path_length: matched_path.path.len(),
                method_specified: !matcher.method.is_empty(),
                header_count: matcher.headers.len(),
                query_count: matcher.query_params.len(),
            };
            (matched_path, score)
        })
        .max_by(|(_, left), (_, right)| left.cmp(right))
}

pub(super) fn best_grpc_rule_match(
    rule: &GrpcRule,
    request: &RequestMeta,
    grpc: Option<&GrpcPath<'_>>,
) -> Option<GrpcRuleScore> {
    if rule.matches.is_empty() {
        return Some(GrpcRuleScore::default());
    }

    rule.matches
        .iter()
        .filter(|matcher| matches_grpc_rule(matcher, request, grpc))
        .map(|matcher| GrpcRuleScore {
            service_specified: !matcher.service.is_empty(),
            method_specified: !matcher.method.is_empty(),
            header_count: matcher.headers.len(),
        })
        .max()
}

pub(super) fn best_hostname_score(
    hostnames: &[String],
    request_host: Option<&str>,
) -> Option<HostnameScore> {
    if hostnames.is_empty() {
        return Some(HostnameScore::default());
    }

    let request_host = request_host?;
    hostnames
        .iter()
        .filter_map(|hostname| hostname_match_score(hostname, request_host))
        .max()
}

pub(super) fn listener_hostname_score(
    listener: &Listener,
    request_host: Option<&str>,
) -> Option<HostnameScore> {
    if listener.hostnames.is_empty() {
        return Some(HostnameScore::default());
    }

    best_hostname_score(&listener.hostnames, request_host)
}

fn hostname_match_score(pattern: &str, request_host: &str) -> Option<HostnameScore> {
    let request_host = super::normalize_host_ref(request_host);
    if let Some(suffix) = pattern.strip_prefix("*.") {
        let suffix = super::normalize_host_ref(suffix);
        return hostname_matches(pattern, request_host).then_some(HostnameScore {
            rank: 1,
            length: suffix.len(),
        });
    }

    let normalized = super::normalize_host_ref(pattern);
    (normalized == request_host).then_some(HostnameScore {
        rank: 2,
        length: normalized.len(),
    })
}

fn http_path_rank(path_type: &str) -> u8 {
    match path_type {
        "Exact" => 3,
        "RegularExpression" => 2,
        _ => 1,
    }
}
