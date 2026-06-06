use super::*;

const RENDEZVOUS_HASH_OFFSET_BASIS: u64 = 0xcbf29ce484222325;
const RENDEZVOUS_HASH_PRIME: u64 = 0x100000001b3;

pub(super) fn listener_attaches_route(listener: &Listener, route: &StreamRoute) -> bool {
    let route_name = format!("{}/{}", route.namespace, route.name);
    listener
        .attached_routes
        .iter()
        .any(|item| item == &route_name)
}

pub(super) fn backend_cluster_name(cluster: &BackendCluster) -> String {
    format!("{}/{}", cluster.namespace, cluster.name)
}

pub(super) fn backend_cluster_service_key(cluster_name: &str) -> Option<(&str, u32)> {
    let (name, port_text) = cluster_name.rsplit_once(':')?;
    let port = parse_decimal_port(port_text)?;
    Some((name, port))
}

pub(super) fn backend_cluster_matches_ref(
    cluster: &BackendCluster,
    namespace: &str,
    name: &str,
    port: u32,
) -> bool {
    cluster.namespace == namespace
        && backend_cluster_name_matches_ref(cluster.name.as_str(), name, port)
}

fn backend_cluster_name_matches_ref(cluster_name: &str, name: &str, port: u32) -> bool {
    let Some((cluster_name, cluster_port)) = cluster_name.rsplit_once(':') else {
        return false;
    };

    cluster_name == name && port_matches_decimal(cluster_port, port)
}

fn port_matches_decimal(port: &str, expected: u32) -> bool {
    parse_decimal_port(port).is_some_and(|parsed| parsed == expected)
}

fn decimal_digit_count(mut value: u32) -> usize {
    let mut count = 1usize;
    while value >= 10 {
        value /= 10;
        count += 1;
    }
    count
}

fn parse_decimal_port(port: &str) -> Option<u32> {
    if port.is_empty() {
        return None;
    }

    let mut parsed = 0u32;
    for digit in port.bytes() {
        if !digit.is_ascii_digit() {
            return None;
        }

        let digit = u32::from(digit - b'0');
        let next = parsed
            .checked_mul(10)
            .and_then(|value| value.checked_add(digit))?;
        parsed = next;
    }

    (port.len() == decimal_digit_count(parsed)).then_some(parsed)
}

pub(super) fn route_kind_for_listener(protocol: &str) -> Option<RouteKind> {
    match protocol {
        "LISTENER_PROTOCOL_TCP" | "TCP" => Some(RouteKind::Tcp),
        "LISTENER_PROTOCOL_UDP" | "UDP" => Some(RouteKind::Udp),
        "LISTENER_PROTOCOL_TLS"
        | "LISTENER_PROTOCOL_TLS_PASSTHROUGH"
        | "TLS"
        | "TLS_PASSTHROUGH" => Some(RouteKind::Tls),
        _ => None,
    }
}

pub(super) fn consistent_hash_key(
    policy: &LoadBalancingPolicy,
    request: &RequestMeta,
) -> Option<String> {
    let hash = policy.consistent_hash.as_ref()?;
    match hash.key_type.as_str() {
        "SourceIP" => request.source_ip.clone(),
        "Header" => request_header_value(request, hash.header_name.as_str()),
        "Hostname" => request.host.clone(),
        _ => None,
    }
}

fn request_header_value(request: &RequestMeta, header_name: &str) -> Option<String> {
    if header_name.is_empty() {
        return None;
    }

    request
        .headers
        .get(header_name)
        .and_then(|values| values.first())
        .cloned()
        .or_else(|| {
            request
                .headers
                .iter()
                .find(|(name, _)| name.eq_ignore_ascii_case(header_name))
                .and_then(|(_, values)| values.first())
                .cloned()
        })
}

pub(super) fn weighted_rendezvous_score(hash_key: &str, backend_name: &str, weight: f64) -> f64 {
    let unit = hash_unit_interval(&[hash_key, backend_name]);
    weight / (-unit.ln())
}

fn hash_unit_interval(parts: &[&str]) -> f64 {
    let value = rendezvous_hash(parts);
    (value as f64 + 1.0) / (u64::MAX as f64 + 2.0)
}

pub(super) fn rendezvous_hash(parts: &[&str]) -> u64 {
    let mut hash = RENDEZVOUS_HASH_OFFSET_BASIS;
    for part in parts {
        hash_rendezvous_part(&mut hash, part.as_bytes());
    }
    hash
}

pub(super) fn rendezvous_hash_endpoint(
    hash_key: &str,
    backend_name: &str,
    endpoint: &BackendEndpoint,
) -> u64 {
    let mut hash = RENDEZVOUS_HASH_OFFSET_BASIS;
    hash_rendezvous_part(&mut hash, hash_key.as_bytes());
    hash_rendezvous_part(&mut hash, backend_name.as_bytes());
    hash_rendezvous_part(&mut hash, endpoint.address.as_bytes());
    hash_rendezvous_u32_part(&mut hash, endpoint.port);
    hash
}

fn hash_rendezvous_part(hash: &mut u64, bytes: &[u8]) {
    for byte in bytes {
        *hash ^= u64::from(*byte);
        *hash = hash.wrapping_mul(RENDEZVOUS_HASH_PRIME);
    }
    *hash ^= 0;
    *hash = hash.wrapping_mul(RENDEZVOUS_HASH_PRIME);
}

fn hash_rendezvous_u32_part(hash: &mut u64, value: u32) {
    let mut digits = [0u8; 10];
    let mut cursor = digits.len();
    let mut value = value;

    loop {
        cursor -= 1;
        digits[cursor] = b'0' + (value % 10) as u8;
        value /= 10;
        if value == 0 {
            break;
        }
    }

    hash_rendezvous_part(hash, &digits[cursor..]);
}

pub(super) fn route_kind_from_name(kind: &str) -> Option<RouteKind> {
    match kind {
        "ROUTE_KIND_TCP" | "TCP" => Some(RouteKind::Tcp),
        "ROUTE_KIND_UDP" | "UDP" => Some(RouteKind::Udp),
        "ROUTE_KIND_TLS" | "TLS" => Some(RouteKind::Tls),
        "ROUTE_KIND_HTTP" | "HTTP" => Some(RouteKind::Http),
        "ROUTE_KIND_GRPC" | "GRPC" => Some(RouteKind::Grpc),
        _ => None,
    }
}

pub(super) fn build_http_route_hostname_index(routes: &[HttpRoute]) -> HostnameRouteIndex {
    build_hostname_route_index(routes.iter().map(|route| &route.hostnames))
}

pub(super) fn build_grpc_route_hostname_index(routes: &[GrpcRoute]) -> HostnameRouteIndex {
    build_hostname_route_index(routes.iter().map(|route| &route.hostnames))
}

fn build_hostname_route_index<'a, I>(route_hostnames: I) -> HostnameRouteIndex
where
    I: IntoIterator<Item = &'a Vec<String>>,
{
    let mut index = HostnameRouteIndex::default();

    for (route_index, hostnames) in route_hostnames.into_iter().enumerate() {
        if hostnames.is_empty() {
            index.catch_all.push(route_index);
            continue;
        }

        for hostname in hostnames {
            let normalized = normalize_host_ref(hostname).to_string();
            if let Some(suffix) = normalized.strip_prefix("*.") {
                index
                    .wildcard_suffix
                    .entry(suffix.to_string())
                    .or_default()
                    .push(route_index);
            } else {
                index.exact.entry(normalized).or_default().push(route_index);
            }
        }
    }

    index
}

pub(super) fn build_stream_listener_route_index(
    listeners: &[Listener],
    routes: &[StreamRoute],
) -> HashMap<String, Vec<usize>> {
    let mut route_index = HashMap::<String, Vec<usize>>::new();
    for (index, route) in routes.iter().enumerate() {
        route_index
            .entry(format!("{}/{}", route.namespace, route.name))
            .or_default()
            .push(index);
    }

    let mut listener_index = HashMap::new();
    for listener in listeners {
        let mut attached = Vec::new();
        for route_name in &listener.attached_routes {
            if let Some(indices) = route_index.get(route_name) {
                attached.extend(indices.iter().copied());
            }
        }
        if !attached.is_empty() {
            listener_index.insert(listener.name.clone(), attached);
        }
    }

    listener_index
}

pub(super) fn build_route_attachment_listener_index(
    listeners: &[Listener],
) -> RouteAttachmentListenerIndex {
    RouteAttachmentListenerIndex::from_listeners(listeners)
}

pub(super) fn build_request_materialization_hints(
    snapshot: &Snapshot,
) -> RequestMaterializationHints {
    let http_route_headers = snapshot.http_routes.iter().any(|route| {
        route.rules.iter().any(|rule| {
            rule.matches
                .iter()
                .any(|matcher| !matcher.headers.is_empty())
        })
    });
    let grpc_route_headers = snapshot.grpc_routes.iter().any(|route| {
        route.rules.iter().any(|rule| {
            rule.matches
                .iter()
                .any(|matcher| !matcher.headers.is_empty())
        })
    });
    let route_session_headers = snapshot.http_routes.iter().any(|route| {
        route
            .rules
            .iter()
            .any(|rule| rule.session_persistence.is_some())
    }) || snapshot.grpc_routes.iter().any(|route| {
        route
            .rules
            .iter()
            .any(|rule| rule.session_persistence.is_some())
    });
    let policy_session_headers = snapshot
        .backend_policies
        .values()
        .any(|policy| policy.session_persistence.is_some());
    let backend_hash_headers = snapshot.backend_policies.values().any(|policy| {
        policy
            .load_balancing
            .as_ref()
            .filter(|load_balancing| load_balancing.policy_type == "ConsistentHash")
            .and_then(|load_balancing| load_balancing.consistent_hash.as_ref())
            .is_some_and(|hash| hash.key_type == "Header")
    });
    let backend_hash_source_ip = snapshot.backend_policies.values().any(|policy| {
        policy
            .load_balancing
            .as_ref()
            .filter(|load_balancing| load_balancing.policy_type == "ConsistentHash")
            .and_then(|load_balancing| load_balancing.consistent_hash.as_ref())
            .is_some_and(|hash| hash.key_type == "SourceIP")
    });
    let mesh_source_ip = !snapshot.workloads.is_empty();

    RequestMaterializationHints {
        http_route_headers,
        grpc_route_headers,
        session_headers: route_session_headers || policy_session_headers,
        backend_hash_headers,
        source_ip: backend_hash_source_ip || mesh_source_ip,
    }
}

pub(super) fn listener_frontend_client_certificate_requirement(
    listener: &Listener,
) -> FrontendClientCertificateRequirement {
    let Some(validation) = listener
        .tls
        .as_ref()
        .and_then(|tls| tls.frontend_validation.as_ref())
    else {
        return FrontendClientCertificateRequirement::None;
    };

    match validation.mode.trim() {
        FRONTEND_VALIDATION_REJECT_MODE => FrontendClientCertificateRequirement::Reject,
        FRONTEND_VALIDATION_ALLOW_INSECURE_FALLBACK_MODE => {
            FrontendClientCertificateRequirement::None
        }
        _ if !validation.ca_pems.is_empty() => FrontendClientCertificateRequirement::Require,
        _ => FrontendClientCertificateRequirement::None,
    }
}

pub(super) fn stream_listener_server_name_score(
    listener: &Listener,
    server_name: Option<&str>,
) -> Option<StreamMatchScore> {
    if listener.hostnames.is_empty() {
        return Some(StreamMatchScore::default());
    }

    let server_name = server_name.map(normalize_host_ref)?;

    listener
        .hostnames
        .iter()
        .filter_map(|hostname| stream_hostname_match_score(hostname, server_name))
        .max()
}

pub(super) fn stream_hostname_match_score(
    pattern: &str,
    server_name: &str,
) -> Option<StreamMatchScore> {
    let pattern = normalize_host_ref(pattern);
    if let Some(suffix) = pattern.strip_prefix("*.") {
        return hostname_matches(pattern, server_name).then_some(StreamMatchScore {
            hostname_rank: 1,
            hostname_length: suffix.len(),
        });
    }

    (pattern == server_name).then_some(StreamMatchScore {
        hostname_rank: 2,
        hostname_length: pattern.len(),
    })
}

impl SelectedHttpRoute {
    pub(super) fn into_backend(self) -> Option<SelectedBackend> {
        Some(SelectedBackend {
            route_kind: RouteKind::Http,
            route_name: self.route_name,
            route_namespace: self.route_namespace,
            rule_index: self.rule_index,
            route_annotations: self.route_annotations,
            listener_name: self.listener_name,
            listener_protocol: self.listener_protocol,
            backend: self.backend?,
            backend_name: self.backend_name?,
            filters: self.filters,
            matched_http_path: Some(self.matched_http_path),
            timeouts: self.timeouts,
            retry: self.retry,
            session_persistence: self.session_persistence,
            backend_tls: self.backend_tls,
        })
    }
}
