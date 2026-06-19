use std::sync::Arc;

use crate::{
    BackendEndpoint, Listener, RouteKind, SelectedBackend, Snapshot, StreamRoute, TlsRouteMode,
    best_stream_rule_match_with_tls_mode,
};

use crate::http_fast_path::CompiledHttpFastBackendRef;

#[derive(Debug, Clone, Default)]
pub struct StreamFastPathPlan {
    routes: Vec<Option<CompiledStreamFastRoute>>,
    route_count: usize,
    eligible_rule_count: usize,
    compiled_backend_ref_count: usize,
}

#[derive(Debug, Clone)]
struct CompiledStreamFastRoute {
    rules: Vec<CompiledStreamFastRule>,
}

#[derive(Debug, Clone)]
struct CompiledStreamFastRule {
    rule_index: usize,
    backend_refs: Vec<CompiledHttpFastBackendRef>,
}

struct StreamFastCandidate {
    route_index: usize,
    rule_index: usize,
    selected: CompiledStreamBackendSelection,
    score: crate::StreamMatchScore,
}

struct CompiledStreamBackendSelection {
    endpoint: BackendEndpoint,
    backend_name: Arc<str>,
}

impl StreamFastPathPlan {
    pub fn build(snapshot: &Snapshot) -> Self {
        let mut routes = vec![None; snapshot.stream_routes.len()];
        let mut route_count = 0usize;
        let mut eligible_rule_count = 0usize;
        let mut compiled_backend_ref_count = 0usize;

        for (route_index, route) in snapshot.stream_routes.iter().enumerate() {
            let rules = route
                .rules
                .iter()
                .enumerate()
                .filter_map(|(rule_index, rule)| {
                    let backend_refs = rule
                        .backend_refs
                        .iter()
                        .filter_map(|backend_ref| {
                            snapshot.compile_http_fast_backend_ref(backend_ref)
                        })
                        .collect::<Vec<_>>();

                    (!backend_refs.is_empty()).then_some(CompiledStreamFastRule {
                        rule_index,
                        backend_refs,
                    })
                })
                .collect::<Vec<_>>();

            if rules.is_empty() {
                continue;
            }

            route_count += 1;
            eligible_rule_count += rules.len();
            compiled_backend_ref_count += rules
                .iter()
                .map(|rule| rule.backend_refs.len())
                .sum::<usize>();
            routes[route_index] = Some(CompiledStreamFastRoute { rules });
        }

        Self {
            routes,
            route_count,
            eligible_rule_count,
            compiled_backend_ref_count,
        }
    }

    pub fn route_count(&self) -> usize {
        self.route_count
    }

    pub fn eligible_rule_count(&self) -> usize {
        self.eligible_rule_count
    }

    pub fn compiled_backend_ref_count(&self) -> usize {
        self.compiled_backend_ref_count
    }

    pub(crate) fn select(
        &self,
        snapshot: &Snapshot,
        listener: &Listener,
        expected_kind: RouteKind,
        server_name: Option<&str>,
        tls_mode: Option<TlsRouteMode>,
    ) -> Option<SelectedBackend> {
        let mut best: Option<StreamFastCandidate> = None;
        let route_indices = snapshot
            .stream_listener_route_index
            .get(listener.name.as_str())?;

        for route_index in route_indices {
            let route = snapshot.stream_routes.get(*route_index)?;
            if stream_route_kind(route) != Some(expected_kind) {
                continue;
            }
            let Some(compiled_route) = self.routes.get(*route_index).and_then(Option::as_ref)
            else {
                continue;
            };

            for compiled_rule in &compiled_route.rules {
                let rule = route.rules.get(compiled_rule.rule_index)?;
                let Some(score) = best_stream_rule_match_with_tls_mode(
                    &rule.matches,
                    listener.port,
                    server_name,
                    tls_mode,
                ) else {
                    continue;
                };
                if best
                    .as_ref()
                    .is_some_and(|candidate| score <= candidate.score)
                {
                    continue;
                }

                let Some(selected) = snapshot
                    .select_compiled_http_fast_backend(&compiled_rule.backend_refs)
                    .map(|selected| CompiledStreamBackendSelection {
                        endpoint: selected.endpoint,
                        backend_name: Arc::clone(&selected.backend_name),
                    })
                else {
                    continue;
                };

                best = Some(StreamFastCandidate {
                    route_index: *route_index,
                    rule_index: compiled_rule.rule_index,
                    selected,
                    score,
                });
            }
        }

        best.and_then(|candidate| {
            let route = snapshot.stream_routes.get(candidate.route_index)?;
            Some(SelectedBackend {
                route_kind: expected_kind,
                route_name: route.name.clone(),
                route_namespace: route.namespace.clone(),
                rule_index: Some(candidate.rule_index),
                route_annotations: route.annotations.clone(),
                listener_name: listener.name.clone(),
                listener_protocol: listener.protocol.clone(),
                backend: candidate.selected.endpoint,
                backend_name: candidate.selected.backend_name.to_string(),
                filters: Vec::new(),
                matched_http_path: None,
                timeouts: None,
                retry: None,
                session_persistence: None,
                backend_tls: None,
            })
        })
    }
}

fn stream_route_kind(route: &StreamRoute) -> Option<RouteKind> {
    match route.kind.as_str() {
        "ROUTE_KIND_TCP" | "TCPRoute" | "TCP" => Some(RouteKind::Tcp),
        "ROUTE_KIND_UDP" | "UDPRoute" | "UDP" => Some(RouteKind::Udp),
        "ROUTE_KIND_TLS" | "TLSRoute" | "TLS" => Some(RouteKind::Tls),
        _ => None,
    }
}
