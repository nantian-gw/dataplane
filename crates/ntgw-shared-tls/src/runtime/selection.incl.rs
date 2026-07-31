fn shared_tls_listener_failures(snapshot: &Snapshot, message: &str) -> Vec<RuntimeListenerFailure> {
    snapshot
        .listeners
        .iter()
        .filter(|listener| shared_tls_listener_protocol(listener))
        .map(|listener| RuntimeListenerFailure {
            listener: listener.name.clone(),
            message: message.to_string(),
        })
        .collect()
}

fn bind_listener_names(bind: &PlannedSharedTlsBind) -> Vec<String> {
    bind.terminate
        .as_ref()
        .into_iter()
        .flat_map(|surface| surface.listener_names.iter().cloned())
        .chain(
            bind.passthrough
                .as_ref()
                .into_iter()
                .flat_map(|surface| surface.listener_names.iter().cloned()),
        )
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn build_runtime_http_app(
    snapshot: SharedSnapshot,
    config: &ReloadableRuntimeConfig,
    traffic: SharedTrafficStats,
    overload: SharedOverloadStats,
    circuit_breaker: Arc<RwLock<HttpCircuitBreakerController>>,
    rate_limit: Arc<RwLock<HttpRateLimitController>>,
    retry_budget: Arc<RwLock<RetryBudgetController>>,
) -> Result<AcceptedHttpApp, SharedTlsError> {
    let http_config = config.http.clone();
    let circuit_breaker = circuit_breaker
        .read()
        .clone();
    let rate_limit = rate_limit
        .read()
        .clone();
    let retry_budget = retry_budget
        .read()
        .clone();
    build_http_app(
        snapshot,
        http_config.runtime.clone(),
        http_config.access_log.clone(),
        http_config.session_persistence.clone(),
        traffic,
        overload,
        circuit_breaker,
        rate_limit,
        retry_budget,
        None,
    )
    .map_err(Into::into)
}

#[derive(Clone, Copy, Debug, Default, Eq, Ord, PartialEq, PartialOrd)]
struct TlsListenerScore {
    rank: u8,
    length: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct SharedTlsListenerMatch {
    listener_name: String,
    score: TlsListenerScore,
    has_route: bool,
}

fn select_shared_tls_listeners(
    snapshot: &SharedSnapshot,
    passthrough_listener_names: &[String],
    terminate_listener_names: &[String],
    server_name: Option<&str>,
) -> (
    Option<SharedTlsListenerMatch>,
    Option<SharedTlsListenerMatch>,
) {
    let current = snapshot.load();
    (
        select_listener_match(
            &current,
            passthrough_listener_names,
            server_name,
            listener_has_matching_passthrough_tls_route,
        ),
        select_listener_match(
            &current,
            terminate_listener_names,
            server_name,
            listener_has_matching_terminate_route,
        ),
    )
}

#[cfg(test)]
fn select_passthrough_listener(
    snapshot: &SharedSnapshot,
    listener_names: &[String],
    server_name: Option<&str>,
) -> Option<String> {
    let current = snapshot.load();
    select_listener_match(
        &current,
        listener_names,
        server_name,
        listener_has_matching_passthrough_tls_route,
    )
    .map(|item| item.listener_name)
}

#[cfg(test)]
fn select_terminate_listener(
    snapshot: &SharedSnapshot,
    listener_names: &[String],
    server_name: Option<&str>,
) -> Option<String> {
    let current = snapshot.load();
    select_listener_match(
        &current,
        listener_names,
        server_name,
        listener_has_matching_terminate_route,
    )
    .filter(|item| item.has_route)
    .map(|item| item.listener_name)
}

fn select_listener_match(
    snapshot: &Snapshot,
    listener_names: &[String],
    server_name: Option<&str>,
    has_matching_route: fn(&Snapshot, &Listener, Option<&str>) -> bool,
) -> Option<SharedTlsListenerMatch> {
    let mut selected: Option<SharedTlsListenerMatch> = None;

    for listener_name in listener_names {
        let Some(listener) = snapshot
            .listeners
            .iter()
            .find(|listener| listener.name == *listener_name)
        else {
            continue;
        };
        let Some(score) = listener_hostname_score(&listener.hostnames, server_name) else {
            continue;
        };

        let has_route = has_matching_route(snapshot, listener, server_name);
        if selected.as_ref().is_some_and(|current| {
            current.score > score || (current.score == score && (current.has_route || !has_route))
        }) {
            continue;
        }

        selected = Some(SharedTlsListenerMatch {
            listener_name: listener.name.clone(),
            score,
            has_route,
        });
    }

    selected
}

fn listener_has_matching_passthrough_tls_route(
    snapshot: &Snapshot,
    listener: &Listener,
    server_name: Option<&str>,
) -> bool {
    listener_has_matching_tls_route_with_mode(
        snapshot,
        listener,
        server_name,
        TlsRouteMode::Passthrough,
    )
}

fn listener_has_matching_terminated_tls_route(
    snapshot: &Snapshot,
    listener: &Listener,
    server_name: Option<&str>,
) -> bool {
    listener_has_matching_tls_route_with_mode(
        snapshot,
        listener,
        server_name,
        TlsRouteMode::Terminate,
    )
}

fn listener_has_matching_tls_route_with_mode(
    snapshot: &Snapshot,
    listener: &Listener,
    server_name: Option<&str>,
    mode: TlsRouteMode,
) -> bool {
    listener.attached_routes.iter().any(|route_key| {
        snapshot.stream_routes.iter().any(|route| {
            route_key == &format!("{}/{}", route.namespace, route.name)
                && matches!(
                    route.kind.as_str(),
                    "ROUTE_KIND_TLS" | "TLS" | "TLSRoute" | "ROUTE_KIND_TLSROUTE"
                )
                && route.rules.iter().any(|rule| {
                    stream_rule_matches_listener_with_mode(
                        rule.matches.as_slice(),
                        listener.port,
                        server_name,
                        mode,
                    )
                })
        })
    })
}

fn listener_has_matching_terminate_route(
    snapshot: &Snapshot,
    listener: &Listener,
    server_name: Option<&str>,
) -> bool {
    listener_has_matching_http_route(snapshot, listener, server_name)
        || listener_has_matching_terminated_tls_route(snapshot, listener, server_name)
}

fn terminate_match_uses_tls_stream_route(
    snapshot: &SharedSnapshot,
    listener_name: &str,
    server_name: Option<&str>,
) -> bool {
    let current = snapshot.load();
    let Some(listener) = current
        .listeners
        .iter()
        .find(|listener| listener.name == listener_name)
    else {
        return false;
    };
    listener_has_matching_terminated_tls_route(&current, listener, server_name)
}

fn listener_has_matching_http_route(
    snapshot: &Snapshot,
    listener: &Listener,
    server_name: Option<&str>,
) -> bool {
    listener.attached_routes.iter().any(|route_key| {
        snapshot.http_routes.iter().any(|route| {
            route_key == &format!("{}/{}", route.namespace, route.name)
                && route_hostnames_match(route.hostnames.as_slice(), server_name)
        })
    })
}

fn stream_rule_matches_listener_with_mode(
    matches: &[ntgw_ir::StreamMatch],
    listener_port: u32,
    server_name: Option<&str>,
    mode: TlsRouteMode,
) -> bool {
    if matches.is_empty() {
        return mode == TlsRouteMode::Passthrough;
    }

    matches.iter().any(|item| {
        if item.mode != mode {
            return false;
        }
        if item.port != 0 && item.port != listener_port {
            return false;
        }
        if item.sni_hostname.is_empty() {
            return true;
        }
        route_hostnames_match(std::slice::from_ref(&item.sni_hostname), server_name)
    })
}

fn listener_hostname_score(
    hostnames: &[String],
    server_name: Option<&str>,
) -> Option<TlsListenerScore> {
    if hostnames.is_empty() {
        return Some(TlsListenerScore::default());
    }

    let server_name = normalize_tls_server_name(server_name?);
    if server_name.is_empty() {
        return None;
    }

    hostnames
        .iter()
        .filter_map(|hostname| hostname_match_score(hostname, server_name.as_str()))
        .max()
}

fn route_hostnames_match(hostnames: &[String], server_name: Option<&str>) -> bool {
    if hostnames.is_empty() {
        return true;
    }

    let Some(server_name) = server_name.map(normalize_tls_server_name) else {
        return false;
    };
    if server_name.is_empty() {
        return false;
    }

    hostnames
        .iter()
        .any(|hostname| hostname_matches(hostname, server_name.as_str()))
}

fn hostname_match_score(pattern: &str, server_name: &str) -> Option<TlsListenerScore> {
    let normalized = normalize_tls_server_name(pattern);
    if normalized.is_empty() {
        return None;
    }

    if let Some(suffix) = normalized.strip_prefix("*.") {
        return hostname_matches(normalized.as_str(), server_name).then_some(TlsListenerScore {
            rank: 1,
            length: suffix.len(),
        });
    }

    (normalized == server_name).then_some(TlsListenerScore {
        rank: 2,
        length: normalized.len(),
    })
}

fn normalize_tls_server_name(value: &str) -> String {
    value.trim().trim_end_matches('.').to_ascii_lowercase()
}

fn hostname_matches(pattern: &str, host: &str) -> bool {
    let pattern = normalize_tls_server_name(pattern);
    let host = normalize_tls_server_name(host);
    if pattern.is_empty() || host.is_empty() {
        return false;
    }
    if pattern == host {
        return true;
    }

    let Some(suffix) = pattern.strip_prefix("*.") else {
        return false;
    };
    host != suffix
        && host.ends_with(suffix)
        && host
            .as_bytes()
            .get(host.len().saturating_sub(suffix.len() + 1))
            .is_some_and(|item| *item == b'.')
}

