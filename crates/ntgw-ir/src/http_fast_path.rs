use std::{collections::BTreeMap, sync::Arc};

use crate::{
    BackendEndpoint, BackendRef, HttpMatch, HttpRule, Listener, MatchedHttpPath, RouteKind,
    RuntimeId, SelectedBackendRuntimeIds, Snapshot, default_http_path_match, hostname_matches,
    mesh, normalize_host_ref, normalize_http_path_match,
};

#[derive(Debug, Clone, Copy)]
pub struct HttpFastPathRequest<'a> {
    pub host: Option<&'a str>,
    pub port: u32,
    pub path: &'a str,
    pub method: &'a str,
    pub is_grpc: bool,
}

#[derive(Debug, Clone)]
pub struct CompiledSelectedHttpBackend {
    pub route_kind: RouteKind,
    pub route_name: Arc<str>,
    pub route_namespace: Arc<str>,
    pub rule_index: Option<usize>,
    pub route_annotations: Arc<BTreeMap<String, String>>,
    pub listener_name: Arc<str>,
    pub listener_protocol: Arc<str>,
    pub backend: BackendEndpoint,
    pub backend_name: Arc<str>,
    pub matched_http_path: MatchedHttpPath,
    pub runtime_ids: SelectedBackendRuntimeIds,
}

#[derive(Debug, Clone, Default)]
pub struct HttpFastPathPlan {
    routes: Vec<CompiledHttpFastRoute>,
    listener_labels: Vec<(Arc<str>, Arc<str>)>,
    selection_safe: bool,
}

#[derive(Debug, Clone)]
struct CompiledHttpFastRoute {
    route_index: usize,
    route_name: Arc<str>,
    route_namespace: Arc<str>,
    route_annotations: Arc<BTreeMap<String, String>>,
    eligible_rules: Vec<CompiledHttpFastRule>,
}

#[derive(Debug, Clone)]
struct CompiledHttpFastRule {
    rule_index: usize,
    runtime_ids: SelectedBackendRuntimeIds,
    backend_refs: Vec<CompiledHttpFastBackendRef>,
}

#[derive(Debug, Clone)]
pub(crate) struct CompiledHttpFastBackendRef {
    pub(crate) backend_index: usize,
    pub(crate) backend_name: Arc<str>,
    pub(crate) weight: u32,
    pub(crate) backend_runtime_id: Option<RuntimeId>,
    pub(crate) endpoint_runtime_ids: Vec<Option<RuntimeId>>,
}

#[derive(Debug, Clone)]
pub(crate) struct CompiledHttpFastBackendSelection {
    pub(crate) endpoint: BackendEndpoint,
    pub(crate) backend_name: Arc<str>,
    pub(crate) backend_runtime_id: Option<RuntimeId>,
    pub(crate) endpoint_runtime_id: Option<RuntimeId>,
}

#[derive(Clone, Copy, Debug, Default, Eq, Ord, PartialEq, PartialOrd)]
struct FastHostnameScore {
    rank: u8,
    length: usize,
}

#[derive(Clone, Copy, Debug, Default, Eq, Ord, PartialEq, PartialOrd)]
struct FastHttpRuleScore {
    path_rank: u8,
    path_length: usize,
    method_specified: bool,
}

#[derive(Clone, Copy, Debug, Default, Eq, Ord, PartialEq, PartialOrd)]
struct FastCandidateScore {
    listener_host: FastHostnameScore,
    route_host: FastHostnameScore,
    rule: FastHttpRuleScore,
}

#[derive(Clone, Copy, Debug)]
struct FastMatchedListener {
    listener_index: usize,
    host_score: FastHostnameScore,
}

#[derive(Debug, Default)]
struct FastCandidateListeners {
    listeners: Vec<FastMatchedListener>,
    enforce_attachments: bool,
}

struct FastCandidate {
    route_name: Arc<str>,
    route_namespace: Arc<str>,
    rule_index: usize,
    route_annotations: Arc<BTreeMap<String, String>>,
    listener_index: Option<usize>,
    matched_http_path: MatchedHttpPath,
    selected: CompiledHttpFastBackendSelection,
    runtime_ids: SelectedBackendRuntimeIds,
    score: FastCandidateScore,
}

impl HttpFastPathPlan {
    pub fn build(snapshot: &Snapshot) -> Self {
        let mut selection_safe = true;
        let routes = snapshot
            .http_routes
            .iter()
            .enumerate()
            .filter_map(|(route_index, route)| {
                let eligible_rules = route
                    .rules
                    .iter()
                    .enumerate()
                    .filter_map(|(rule_index, rule)| {
                        if !http_rule_is_fast_path_selection_safe(rule, snapshot) {
                            selection_safe = false;
                            return None;
                        }

                        let Some(backend_refs) = compile_http_fast_backend_refs(rule, snapshot)
                        else {
                            if http_rule_has_selectable_backend(rule) {
                                selection_safe = false;
                            }
                            return None;
                        };

                        Some(CompiledHttpFastRule {
                            rule_index,
                            runtime_ids: SelectedBackendRuntimeIds {
                                route: snapshot
                                    .http_route_runtime_id(&route.namespace, &route.name),
                                rule: snapshot.http_rule_runtime_id(
                                    &route.namespace,
                                    &route.name,
                                    rule_index,
                                ),
                                ..SelectedBackendRuntimeIds::default()
                            },
                            backend_refs,
                        })
                    })
                    .collect::<Vec<_>>();

                (!eligible_rules.is_empty()).then_some(CompiledHttpFastRoute {
                    route_index,
                    route_name: Arc::from(route.name.as_str()),
                    route_namespace: Arc::from(route.namespace.as_str()),
                    route_annotations: Arc::new(route.annotations.clone()),
                    eligible_rules,
                })
            })
            .collect();

        let listener_labels = snapshot
            .listeners
            .iter()
            .map(|listener| {
                (
                    Arc::from(listener.name.as_str()),
                    Arc::from(listener.protocol.as_str()),
                )
            })
            .collect();

        Self {
            routes,
            listener_labels,
            selection_safe,
        }
    }

    pub fn route_count(&self) -> usize {
        self.routes.len()
    }

    pub fn eligible_rule_count(&self) -> usize {
        self.routes
            .iter()
            .map(|route| route.eligible_rules.len())
            .sum()
    }

    pub fn compiled_backend_ref_count(&self) -> usize {
        self.routes
            .iter()
            .flat_map(|route| &route.eligible_rules)
            .map(|rule| rule.backend_refs.len())
            .sum()
    }

    pub fn select(
        &self,
        snapshot: &Snapshot,
        request: HttpFastPathRequest<'_>,
    ) -> Option<CompiledSelectedHttpBackend> {
        if !self.selection_safe
            || request.is_grpc
            || snapshot.request_materialization.requires_full_headers()
            || snapshot.request_materialization.source_ip
        {
            return None;
        }

        let request_path = request_path_without_query(request.path);
        let request_host = request.host.map(normalize_host_ref);
        let listeners = fast_matched_listeners(snapshot, request_host, request.port);
        let mut best: Option<FastCandidate> = None;

        for compiled_route in &self.routes {
            let route = snapshot.http_routes.get(compiled_route.route_index)?;
            let listener_match =
                fast_route_listener_match(snapshot, &listeners, &route.namespace, &route.name);
            if listeners.enforce_attachments && listener_match.is_none() {
                continue;
            }

            if let Some(listener_match) = listener_match {
                let listener = snapshot.listeners.get(listener_match.listener_index)?;
                if !mesh::route_accepts_service_frontend(
                    snapshot,
                    &route.parent_refs,
                    &route.namespace,
                    listener,
                    None,
                ) {
                    continue;
                }
            }

            let Some(route_host_score) = fast_best_hostname_score(&route.hostnames, request_host)
            else {
                continue;
            };

            for compiled_rule in &compiled_route.eligible_rules {
                let rule = route.rules.get(compiled_rule.rule_index)?;
                let Some((matched_http_path, rule_score)) =
                    fast_best_http_rule_match(rule, request_path, request.method)
                else {
                    continue;
                };

                let score = FastCandidateScore {
                    listener_host: listener_match
                        .map(|item| item.host_score)
                        .unwrap_or_default(),
                    route_host: route_host_score,
                    rule: rule_score,
                };
                if best.as_ref().is_some_and(|current| score <= current.score) {
                    continue;
                }
                if listener_match
                    .and_then(|listener_match| {
                        snapshot.listeners.get(listener_match.listener_index)
                    })
                    .is_some_and(|listener| listener.backend_tls.is_some())
                {
                    return None;
                }

                let selected =
                    snapshot.select_compiled_http_fast_backend(&compiled_rule.backend_refs)?;
                let runtime_ids = SelectedBackendRuntimeIds {
                    listener: listener_match.and_then(|listener_match| {
                        snapshot
                            .listeners
                            .get(listener_match.listener_index)
                            .and_then(|listener| snapshot.listener_runtime_id(&listener.name))
                    }),
                    route: compiled_rule.runtime_ids.route,
                    rule: compiled_rule.runtime_ids.rule,
                    backend: selected.backend_runtime_id,
                    endpoint: selected.endpoint_runtime_id,
                };

                best = Some(FastCandidate {
                    route_name: Arc::clone(&compiled_route.route_name),
                    route_namespace: Arc::clone(&compiled_route.route_namespace),
                    rule_index: compiled_rule.rule_index,
                    route_annotations: Arc::clone(&compiled_route.route_annotations),
                    listener_index: listener_match.map(|item| item.listener_index),
                    matched_http_path,
                    selected,
                    runtime_ids,
                    score,
                });
            }
        }

        best.map(|candidate| {
            let (listener_name, listener_protocol) = candidate
                .listener_index
                .and_then(|index| self.listener_labels.get(index))
                .map(|(name, protocol)| (Arc::clone(name), Arc::clone(protocol)))
                .unwrap_or_else(|| (Arc::from(""), Arc::from("")));

            CompiledSelectedHttpBackend {
                route_kind: RouteKind::Http,
                route_name: candidate.route_name,
                route_namespace: candidate.route_namespace,
                rule_index: Some(candidate.rule_index),
                route_annotations: candidate.route_annotations,
                listener_name,
                listener_protocol,
                backend: candidate.selected.endpoint,
                backend_name: candidate.selected.backend_name,
                matched_http_path: candidate.matched_http_path,
                runtime_ids: candidate.runtime_ids,
            }
        })
    }
}

fn http_rule_is_fast_path_selection_safe(rule: &HttpRule, snapshot: &Snapshot) -> bool {
    rule.timeouts.is_none()
        && rule.retry.is_none()
        && rule.session_persistence.is_none()
        && rule.filters.is_empty()
        && rule.matches.iter().all(http_match_is_fast_path_eligible)
        && rule
            .backend_refs
            .iter()
            .filter(|backend_ref| backend_ref.weight > 0)
            .all(|backend_ref| backend_ref_is_fast_path_eligible(backend_ref, snapshot))
}

fn http_rule_has_selectable_backend(rule: &HttpRule) -> bool {
    rule.backend_refs
        .iter()
        .any(|backend_ref| backend_ref.weight > 0)
}

fn compile_http_fast_backend_refs(
    rule: &HttpRule,
    snapshot: &Snapshot,
) -> Option<Vec<CompiledHttpFastBackendRef>> {
    let compiled = rule
        .backend_refs
        .iter()
        .filter(|backend_ref| backend_ref.weight > 0)
        .map(|backend_ref| snapshot.compile_http_fast_backend_ref(backend_ref))
        .collect::<Option<Vec<_>>>()?;

    (!compiled.is_empty()).then_some(compiled)
}

fn http_match_is_fast_path_eligible(matcher: &HttpMatch) -> bool {
    matcher.headers.is_empty() && matcher.query_params.is_empty()
}

fn backend_ref_is_fast_path_eligible(backend_ref: &BackendRef, snapshot: &Snapshot) -> bool {
    if !backend_ref.filters.is_empty() {
        return false;
    }

    let backend_name = format!(
        "{}/{}:{}",
        backend_ref.namespace, backend_ref.name, backend_ref.port
    );
    snapshot.backend_policy(&backend_name).is_none_or(|policy| {
        policy.session_persistence.is_none()
            && policy
                .load_balancing
                .as_ref()
                .is_none_or(|load_balancing| load_balancing.policy_type != "ConsistentHash")
    })
}

fn fast_matched_listeners(
    snapshot: &Snapshot,
    request_host: Option<&str>,
    request_port: u32,
) -> FastCandidateListeners {
    let mut saw_candidate_listener = false;
    let mut best_score = None;
    let mut listeners = Vec::with_capacity(snapshot.listeners.len());

    visit_fast_candidate_listeners(snapshot, request_port, |listener_index, listener| {
        saw_candidate_listener = true;
        let Some(host_score) = fast_best_hostname_score(&listener.hostnames, request_host) else {
            return;
        };

        match best_score {
            Some(score) if host_score < score => {}
            Some(score) if host_score == score => {
                listeners.push(FastMatchedListener {
                    listener_index,
                    host_score,
                });
            }
            _ => {
                best_score = Some(host_score);
                listeners.clear();
                listeners.push(FastMatchedListener {
                    listener_index,
                    host_score,
                });
            }
        }
    });

    FastCandidateListeners {
        listeners,
        enforce_attachments: saw_candidate_listener,
    }
}

pub(crate) fn visit_fast_candidate_listeners<'a>(
    snapshot: &'a Snapshot,
    request_port: u32,
    mut visit: impl FnMut(usize, &'a Listener),
) {
    if !snapshot.runtime_indexes_ready || request_port == 0 {
        for (listener_index, listener) in snapshot.listeners.iter().enumerate() {
            if crate::http_selection::is_http_listener(&listener.protocol)
                && (request_port == 0 || listener.port == request_port)
            {
                visit(listener_index, listener);
            }
        }
        return;
    }

    let Some(indices) = snapshot.http_listener_port_index.get(&request_port) else {
        return;
    };
    for listener_index in indices {
        if let Some(listener) = snapshot.listeners.get(*listener_index) {
            visit(*listener_index, listener);
        }
    }
}

fn fast_route_listener_match(
    snapshot: &Snapshot,
    listeners: &FastCandidateListeners,
    route_namespace: &str,
    route_name: &str,
) -> Option<FastMatchedListener> {
    if !listeners.enforce_attachments {
        return None;
    }

    if snapshot.runtime_indexes_ready {
        let attached_listener_indices = snapshot
            .route_attachment_listener_index
            .listeners_for_route(route_namespace, route_name)?;

        listeners
            .listeners
            .iter()
            .filter(|listener_match| {
                attached_listener_indices
                    .binary_search(&listener_match.listener_index)
                    .is_ok()
            })
            .copied()
            .max_by(|left, right| left.host_score.cmp(&right.host_score))
    } else {
        listeners
            .listeners
            .iter()
            .filter(|listener_match| {
                snapshot
                    .listeners
                    .get(listener_match.listener_index)
                    .is_some_and(|listener| {
                        listener.attached_routes.iter().any(|attached| {
                            attached_route_matches(attached, route_namespace, route_name)
                        })
                    })
            })
            .copied()
            .max_by(|left, right| left.host_score.cmp(&right.host_score))
    }
}

fn attached_route_matches(attached: &str, route_namespace: &str, route_name: &str) -> bool {
    attached
        .split_once('/')
        .is_some_and(|(namespace, name)| namespace == route_namespace && name == route_name)
}

fn fast_best_hostname_score(
    hostnames: &[String],
    request_host: Option<&str>,
) -> Option<FastHostnameScore> {
    if hostnames.is_empty() {
        return Some(FastHostnameScore::default());
    }

    let request_host = request_host?;
    hostnames
        .iter()
        .filter_map(|hostname| fast_hostname_score(hostname, request_host))
        .max()
}

fn fast_hostname_score(pattern: &str, request_host: &str) -> Option<FastHostnameScore> {
    let request_host = normalize_host_ref(request_host);
    if let Some(suffix) = pattern.strip_prefix("*.") {
        let suffix = normalize_host_ref(suffix);
        return hostname_matches(pattern, request_host).then_some(FastHostnameScore {
            rank: 1,
            length: suffix.len(),
        });
    }

    let normalized = normalize_host_ref(pattern);
    (normalized == request_host).then_some(FastHostnameScore {
        rank: 2,
        length: normalized.len(),
    })
}

fn fast_best_http_rule_match(
    rule: &HttpRule,
    request_path: &str,
    request_method: &str,
) -> Option<(MatchedHttpPath, FastHttpRuleScore)> {
    if rule.matches.is_empty() {
        return Some((default_http_path_match(), FastHttpRuleScore::default()));
    }

    rule.matches
        .iter()
        .filter(|matcher| fast_matches_http_rule(matcher, request_path, request_method))
        .map(|matcher| {
            let matched_path = normalize_http_path_match(matcher);
            let score = FastHttpRuleScore {
                path_rank: fast_http_path_rank(&matched_path.path_type),
                path_length: matched_path.path.len(),
                method_specified: !matcher.method.is_empty(),
            };
            (matched_path, score)
        })
        .max_by(|(_, left), (_, right)| left.cmp(right))
}

fn fast_matches_http_rule(matcher: &HttpMatch, request_path: &str, request_method: &str) -> bool {
    fast_matches_http_path(matcher, request_path)
        && (matcher.method.is_empty() || matcher.method.eq_ignore_ascii_case(request_method))
}

fn fast_matches_http_path(matcher: &HttpMatch, request_path: &str) -> bool {
    if matcher.path.is_empty() {
        return true;
    }

    match matcher.path_type.as_str() {
        "Exact" => request_path == matcher.path,
        "RegularExpression" => matcher
            .compiled_path_regex
            .as_ref()
            .is_some_and(|regex| regex.is_match(request_path)),
        _ => fast_matches_path_prefix(&matcher.path, request_path),
    }
}

fn fast_matches_path_prefix(prefix: &str, path: &str) -> bool {
    if prefix == "/" || path == prefix {
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

fn fast_http_path_rank(path_type: &str) -> u8 {
    match path_type {
        "Exact" => 3,
        "RegularExpression" => 2,
        _ => 1,
    }
}

fn request_path_without_query(path: &str) -> &str {
    path.split_once('?').map(|(path, _)| path).unwrap_or(path)
}
