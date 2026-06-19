use super::*;
use crate::http_fast_path::{CompiledHttpFastBackendRef, CompiledHttpFastBackendSelection};
use std::borrow::Cow;

struct BackendSelectionCandidate<'a> {
    cluster: &'a BackendCluster,
    backend_ref: &'a BackendRef,
    backend_name: Cow<'a, str>,
}

#[derive(Default)]
struct BackendCandidateVisit<'a> {
    candidate_count: usize,
    total_weight: u64,
    saw_invalid_refs: bool,
    saw_unhealthy_backend: bool,
    effective_load_balancing: Option<&'a LoadBalancingPolicy>,
    load_balancing_mismatch: bool,
}

impl<'a> BackendCandidateVisit<'a> {
    fn observe_candidate(
        &mut self,
        backend_ref: &BackendRef,
        load_balancing: Option<&'a LoadBalancingPolicy>,
    ) {
        self.candidate_count += 1;
        self.total_weight += backend_ref.weight as u64;

        if self.load_balancing_mismatch {
            return;
        }

        let Some(load_balancing) = load_balancing else {
            self.effective_load_balancing = None;
            self.load_balancing_mismatch = true;
            return;
        };

        match self.effective_load_balancing {
            Some(current) if current != load_balancing => {
                self.effective_load_balancing = None;
                self.load_balancing_mismatch = true;
            }
            Some(_) => {}
            None => self.effective_load_balancing = Some(load_balancing),
        }
    }

    fn error(&self) -> Option<BackendSelectionError> {
        if self.saw_unhealthy_backend {
            Some(BackendSelectionError::NoHealthyBackends)
        } else if self.saw_invalid_refs {
            Some(BackendSelectionError::InvalidBackendRefs)
        } else {
            None
        }
    }
}

impl Snapshot {
    pub(super) fn resolve_backend_refs(
        &self,
        refs: &[BackendRef],
    ) -> Option<(BackendEndpoint, String)> {
        self.resolve_http_backend_refs(refs)
            .selected
            .map(|item| (item.endpoint, item.backend_name))
    }

    fn resolve_http_backend_refs(&self, refs: &[BackendRef]) -> HttpBackendResolution {
        self.resolve_http_backend_refs_with_session(refs, None, &RequestMeta::default(), &|_| None)
    }

    pub(crate) fn compile_http_fast_backend_ref(
        &self,
        backend_ref: &BackendRef,
    ) -> Option<CompiledHttpFastBackendRef> {
        if backend_ref.weight == 0 || !backend_ref_is_routable(backend_ref) {
            return None;
        }

        let requested_backend_name = format!(
            "{}/{}:{}",
            backend_ref.namespace, backend_ref.name, backend_ref.port
        );
        let backend_index = self
            .backend_index
            .get(requested_backend_name.as_str())
            .copied()?;
        let cluster = self.backends.get(backend_index)?;
        let backend_name = Arc::<str>::from(backend_cluster_name(cluster));
        let endpoint_runtime_ids = cluster
            .endpoints
            .iter()
            .map(|endpoint| self.endpoint_runtime_id(backend_name.as_ref(), endpoint))
            .collect();

        Some(CompiledHttpFastBackendRef {
            backend_index,
            backend_runtime_id: self.backend_runtime_id(backend_name.as_ref()),
            backend_name,
            weight: backend_ref.weight,
            endpoint_runtime_ids,
        })
    }

    pub(crate) fn select_compiled_http_fast_backend(
        &self,
        refs: &[CompiledHttpFastBackendRef],
    ) -> Option<CompiledHttpFastBackendSelection> {
        let now = Instant::now();
        let total_weight = refs
            .iter()
            .filter_map(|compiled| {
                let cluster = self.backends.get(compiled.backend_index)?;
                (self
                    .endpoint_selection_availability(cluster, compiled.backend_name.as_ref(), now)
                    .count
                    > 0)
                .then_some(compiled.weight as u64)
            })
            .sum::<u64>();
        if total_weight == 0 {
            return None;
        }

        let target =
            spread_weighted_target(self.selection_state.next_backend_ticket(), total_weight);
        let mut seen_weight = 0u64;
        for compiled in refs {
            let Some(cluster) = self.backends.get(compiled.backend_index) else {
                continue;
            };
            let availability =
                self.endpoint_selection_availability(cluster, compiled.backend_name.as_ref(), now);
            if availability.count == 0 {
                continue;
            }

            seen_weight += compiled.weight as u64;
            if target < seen_weight {
                return self.select_compiled_http_fast_endpoint(
                    compiled,
                    cluster,
                    availability,
                    now,
                );
            }
        }

        None
    }

    pub(crate) fn resolve_http_backend_refs_with_session<F>(
        &self,
        refs: &[BackendRef],
        route_session: Option<&SessionPersistence>,
        request: &RequestMeta,
        session_resolver: &F,
    ) -> HttpBackendResolution
    where
        F: Fn(&SessionPersistence) -> Option<PersistentSessionTarget>,
    {
        if let Some(selected) = route_session
            .and_then(session_resolver)
            .and_then(|target| self.resolve_persistent_http_backend(refs, &target, None))
        {
            return HttpBackendResolution {
                selected: Some(selected),
                error: None,
            };
        }

        if route_session.is_none()
            && let Some(selected) =
                self.resolve_backend_policy_persistent_http_backend(refs, session_resolver)
        {
            return HttpBackendResolution {
                selected: Some(selected),
                error: None,
            };
        }

        let now = Instant::now();
        let outcome = self.visit_http_backend_candidates(refs, now, |_| true);

        if outcome.candidate_count == 0 {
            return HttpBackendResolution {
                selected: None,
                error: outcome.error(),
            };
        }

        if let Some(selected) = self.resolve_consistent_hash_http_backend(
            refs,
            now,
            outcome.effective_load_balancing,
            route_session,
            request,
        ) {
            return HttpBackendResolution {
                selected: Some(selected),
                error: None,
            };
        }

        let target = spread_weighted_target(
            self.selection_state.next_backend_ticket(),
            outcome.total_weight,
        );
        let mut seen_weight = 0u64;
        let mut resolution = None;
        self.visit_http_backend_candidates(refs, now, |candidate| {
            seen_weight += candidate.backend_ref.weight as u64;
            if target < seen_weight {
                let backend_name = candidate.backend_name;
                let backend_name_ref = backend_name.as_ref();
                let session_persistence = route_session
                    .is_none()
                    .then(|| self.backend_session_persistence(backend_name_ref))
                    .flatten()
                    .cloned();
                let selected = self
                    .select_cluster_endpoint(candidate.cluster, backend_name_ref)
                    .map(|endpoint| ResolvedHttpBackend {
                        endpoint,
                        backend_name: backend_name.into_owned(),
                        filters: candidate.backend_ref.filters.clone(),
                        session_persistence,
                    });
                resolution = Some(HttpBackendResolution {
                    selected,
                    error: None,
                });
                return false;
            }
            true
        });
        if let Some(resolution) = resolution {
            return resolution;
        }

        HttpBackendResolution::default()
    }

    fn resolve_persistent_http_backend(
        &self,
        refs: &[BackendRef],
        target: &PersistentSessionTarget,
        session_persistence: Option<&SessionPersistence>,
    ) -> Option<ResolvedHttpBackend> {
        for backend_ref in refs {
            if backend_ref.weight == 0 || !backend_ref_is_routable(backend_ref) {
                continue;
            }

            let Some((backend_index, cluster)) = self.find_backend_cluster_index(backend_ref)
            else {
                continue;
            };
            let backend_name = self.backend_name_for_index(backend_index, cluster);
            let backend_name_ref = backend_name.as_ref();
            if backend_name_ref != target.backend_name {
                continue;
            }

            let now = Instant::now();
            let availability = self.endpoint_selection_availability(cluster, backend_name_ref, now);
            let endpoint = cluster
                .endpoints
                .iter()
                .find(|endpoint| {
                    self.endpoint_matches_selection_availability(
                        backend_name_ref,
                        endpoint,
                        now,
                        availability,
                    ) && endpoint.address == target.endpoint.address
                        && endpoint.port == target.endpoint.port
                })?
                .clone();

            return Some(ResolvedHttpBackend {
                endpoint,
                backend_name: backend_name.into_owned(),
                filters: backend_ref.filters.clone(),
                session_persistence: session_persistence.cloned(),
            });
        }

        None
    }

    fn resolve_backend_policy_persistent_http_backend<F>(
        &self,
        refs: &[BackendRef],
        session_resolver: &F,
    ) -> Option<ResolvedHttpBackend>
    where
        F: Fn(&SessionPersistence) -> Option<PersistentSessionTarget>,
    {
        for backend_ref in refs {
            if backend_ref.weight == 0 || !backend_ref_is_routable(backend_ref) {
                continue;
            }

            let Some((backend_index, cluster)) = self.find_backend_cluster_index(backend_ref)
            else {
                continue;
            };
            let backend_name = self.backend_name_for_index(backend_index, cluster);
            let Some(session_persistence) = self.backend_session_persistence(backend_name.as_ref())
            else {
                continue;
            };
            let Some(target) = session_resolver(session_persistence) else {
                continue;
            };

            if let Some(selected) =
                self.resolve_persistent_http_backend(refs, &target, Some(session_persistence))
            {
                return Some(selected);
            }
        }

        None
    }

    #[cfg(test)]
    fn collect_http_backend_candidates<'a>(
        &'a self,
        refs: &'a [BackendRef],
        now: Instant,
    ) -> (Vec<BackendSelectionCandidate<'a>>, bool, bool) {
        let mut candidates = Vec::new();
        let outcome = self.visit_http_backend_candidates(refs, now, |candidate| {
            candidates.push(candidate);
            true
        });

        (
            candidates,
            outcome.saw_invalid_refs,
            outcome.saw_unhealthy_backend,
        )
    }

    fn visit_http_backend_candidates<'a>(
        &'a self,
        refs: &'a [BackendRef],
        now: Instant,
        mut visit: impl FnMut(BackendSelectionCandidate<'a>) -> bool,
    ) -> BackendCandidateVisit<'a> {
        let mut outcome = BackendCandidateVisit::default();
        for backend_ref in refs {
            if backend_ref.weight == 0 {
                continue;
            }
            if !backend_ref_is_routable(backend_ref) {
                outcome.saw_invalid_refs = true;
                continue;
            }

            let Some((backend_index, cluster)) = self.find_backend_cluster_index(backend_ref)
            else {
                outcome.saw_invalid_refs = true;
                continue;
            };
            let backend_name = self.backend_name_for_index(backend_index, cluster);
            if self.eligible_endpoint_count(cluster, backend_name.as_ref(), now) == 0 {
                outcome.saw_unhealthy_backend = true;
                continue;
            }

            outcome.observe_candidate(
                backend_ref,
                self.backend_load_balancing(backend_name.as_ref()),
            );
            let candidate = BackendSelectionCandidate {
                cluster,
                backend_ref,
                backend_name,
            };
            if !visit(candidate) {
                return outcome;
            }
        }

        outcome
    }

    fn resolve_consistent_hash_http_backend(
        &self,
        refs: &[BackendRef],
        now: Instant,
        policy: Option<&LoadBalancingPolicy>,
        route_session: Option<&SessionPersistence>,
        request: &RequestMeta,
    ) -> Option<ResolvedHttpBackend> {
        let policy = policy?;
        if policy.policy_type != "ConsistentHash" {
            return None;
        }

        let hash_key = consistent_hash_key(policy, request)?;
        let mut best: Option<(f64, ResolvedHttpBackend)> = None;

        self.visit_http_backend_candidates(refs, now, |candidate| {
            let backend_name_ref = candidate.backend_name.as_ref();
            let Some(endpoint) = self.select_cluster_endpoint_by_hash(
                candidate.cluster,
                backend_name_ref,
                hash_key.as_ref(),
            ) else {
                return true;
            };
            let score = weighted_rendezvous_score(
                hash_key.as_ref(),
                backend_name_ref,
                candidate.backend_ref.weight.max(1) as f64,
            );
            let session_persistence = route_session
                .is_none()
                .then(|| self.backend_session_persistence(backend_name_ref))
                .flatten()
                .cloned();
            let backend_name = candidate.backend_name.into_owned();
            let candidate = ResolvedHttpBackend {
                endpoint,
                backend_name,
                filters: candidate.backend_ref.filters.clone(),
                session_persistence,
            };

            match &best {
                Some((current_score, _)) if score <= *current_score => {}
                _ => best = Some((score, candidate)),
            }
            true
        });

        best.map(|(_, selected)| selected)
    }

    pub fn select_backend_ref(
        &self,
        backend_ref: &BackendRef,
    ) -> Option<(BackendEndpoint, String)> {
        if !backend_ref_is_routable(backend_ref) {
            return None;
        }
        let (backend_index, cluster) = self.find_backend_cluster_index(backend_ref)?;
        let backend_name = self.backend_name_for_index(backend_index, cluster);
        self.select_cluster_endpoint(cluster, backend_name.as_ref())
            .map(|endpoint| (endpoint, backend_name.into_owned()))
    }

    fn find_backend_cluster_index(
        &self,
        backend_ref: &BackendRef,
    ) -> Option<(usize, &BackendCluster)> {
        if !backend_ref_is_routable(backend_ref) {
            return None;
        }

        self.backend_cluster_for_service_index(
            backend_ref.namespace.as_str(),
            backend_ref.name.as_str(),
            backend_ref.port,
        )
    }

    fn backend_session_persistence(&self, backend_name: &str) -> Option<&SessionPersistence> {
        self.backend_policy(backend_name)
            .and_then(|policy| policy.session_persistence.as_ref())
    }

    fn backend_load_balancing(&self, backend_name: &str) -> Option<&LoadBalancingPolicy> {
        self.backend_policy(backend_name)
            .and_then(|policy| policy.load_balancing.as_ref())
    }

    pub(super) fn select_cluster_endpoint(
        &self,
        cluster: &BackendCluster,
        backend_name: &str,
    ) -> Option<BackendEndpoint> {
        let now = Instant::now();
        let availability = self.endpoint_selection_availability(cluster, backend_name, now);
        if availability.count == 0 {
            return None;
        }

        let index =
            (self.selection_state.next_endpoint_ticket() % availability.count as u64) as usize;
        cluster
            .endpoints
            .iter()
            .filter(|endpoint| {
                self.endpoint_matches_selection_availability(
                    backend_name,
                    endpoint,
                    now,
                    availability,
                )
            })
            .nth(index)
            .cloned()
    }

    fn select_cluster_endpoint_by_hash(
        &self,
        cluster: &BackendCluster,
        backend_name: &str,
        hash_key: &str,
    ) -> Option<BackendEndpoint> {
        let now = Instant::now();
        let availability = self.endpoint_selection_availability(cluster, backend_name, now);
        if availability.count == 0 {
            return None;
        }

        let mut best: Option<(u64, BackendEndpoint)> = None;

        for endpoint in &cluster.endpoints {
            if !self.endpoint_matches_selection_availability(
                backend_name,
                endpoint,
                now,
                availability,
            ) {
                continue;
            }

            let score = rendezvous_hash_endpoint(hash_key, backend_name, endpoint);
            match &best {
                Some((current_score, _)) if score <= *current_score => {}
                _ => best = Some((score, endpoint.clone())),
            }
        }

        best.map(|(_, endpoint)| endpoint)
    }

    fn select_compiled_http_fast_endpoint(
        &self,
        compiled: &CompiledHttpFastBackendRef,
        cluster: &BackendCluster,
        availability: EndpointSelectionAvailability,
        now: Instant,
    ) -> Option<CompiledHttpFastBackendSelection> {
        if availability.count == 0 {
            return None;
        }

        let target =
            (self.selection_state.next_endpoint_ticket() % availability.count as u64) as usize;
        let mut seen = 0usize;
        for (endpoint_index, endpoint) in cluster.endpoints.iter().enumerate() {
            if !self.endpoint_matches_selection_availability(
                compiled.backend_name.as_ref(),
                endpoint,
                now,
                availability,
            ) {
                continue;
            }
            if seen == target {
                return Some(CompiledHttpFastBackendSelection {
                    endpoint: endpoint.clone(),
                    backend_name: Arc::clone(&compiled.backend_name),
                    backend_runtime_id: compiled.backend_runtime_id,
                    endpoint_runtime_id: compiled
                        .endpoint_runtime_ids
                        .get(endpoint_index)
                        .copied()
                        .flatten(),
                });
            }
            seen += 1;
        }

        None
    }

    pub(super) fn backend_cluster_by_name(&self, backend_name: &str) -> Option<&BackendCluster> {
        if self.runtime_indexes_ready
            && let Some(cluster) = self
                .backend_index
                .get(backend_name)
                .and_then(|index| self.backends.get(*index))
        {
            return Some(cluster);
        }

        let (namespace, name) = backend_name.split_once('/')?;
        self.backends
            .iter()
            .find(|cluster| cluster.namespace == namespace && cluster.name == name)
    }

    pub(super) fn inherit_endpoint_runtime(&self, previous: &Snapshot) -> EndpointRuntimeStore {
        previous
            .endpoint_runtime
            .inherit_for_backends(&self.backends)
    }

    fn eligible_endpoint_count(
        &self,
        cluster: &BackendCluster,
        backend_name: &str,
        now: Instant,
    ) -> usize {
        self.endpoint_selection_availability(cluster, backend_name, now)
            .count
    }

    fn endpoint_selection_availability(
        &self,
        cluster: &BackendCluster,
        backend_name: &str,
        now: Instant,
    ) -> EndpointSelectionAvailability {
        let mut primary = 0usize;
        let mut last_resort = 0usize;

        for endpoint in &cluster.endpoints {
            match self.endpoint_availability_at(backend_name, endpoint, now) {
                EndpointAvailability::Primary => {
                    primary += 1;
                    last_resort += 1;
                }
                EndpointAvailability::PassiveLastResort => {
                    last_resort += 1;
                }
                EndpointAvailability::Unavailable => {}
            }
        }

        // Passive ejection is only a routing preference when there is an alternative.
        if primary > 0 {
            EndpointSelectionAvailability {
                count: primary,
                include_passive_ejected: false,
            }
        } else {
            EndpointSelectionAvailability {
                count: last_resort,
                include_passive_ejected: true,
            }
        }
    }

    fn endpoint_matches_selection_availability(
        &self,
        backend_name: &str,
        endpoint: &BackendEndpoint,
        now: Instant,
        availability: EndpointSelectionAvailability,
    ) -> bool {
        match self.endpoint_availability_at(backend_name, endpoint, now) {
            EndpointAvailability::Primary => true,
            EndpointAvailability::PassiveLastResort => availability.include_passive_ejected,
            EndpointAvailability::Unavailable => false,
        }
    }

    fn endpoint_availability_at(
        &self,
        backend_name: &str,
        endpoint: &BackendEndpoint,
        now: Instant,
    ) -> EndpointAvailability {
        if !endpoint.healthy {
            return EndpointAvailability::Unavailable;
        }
        if !self.endpoint_runtime.has_tracked_states() {
            return EndpointAvailability::Primary;
        }

        match self
            .endpoint_runtime
            .get_cloned(&EndpointRuntimeKey::new(backend_name, endpoint))
        {
            Some(state) if state.active_unhealthy => EndpointAvailability::Unavailable,
            Some(state) if state.is_ejected_at(now) => EndpointAvailability::PassiveLastResort,
            _ => EndpointAvailability::Primary,
        }
    }

    pub(super) fn endpoint_is_available_at(
        &self,
        backend_name: &str,
        endpoint: &BackendEndpoint,
        now: Instant,
    ) -> bool {
        matches!(
            self.endpoint_availability_at(backend_name, endpoint, now),
            EndpointAvailability::Primary
        )
    }
}

#[derive(Clone, Copy)]
struct EndpointSelectionAvailability {
    count: usize,
    include_passive_ejected: bool,
}

#[derive(Clone, Copy)]
enum EndpointAvailability {
    Primary,
    PassiveLastResort,
    Unavailable,
}

impl Snapshot {
    pub(super) fn should_mirror(&self, filter: &RequestMirrorFilter) -> bool {
        let (numerator, denominator) = match &filter.fraction {
            Some(fraction) => (fraction.numerator, fraction.denominator.max(1)),
            None => (filter.percent.unwrap_or(100), 100),
        };

        if numerator == 0 {
            return false;
        }
        if numerator >= denominator {
            return true;
        }

        (self.selection_state.next_mirror_ticket() % denominator as u64) < numerator as u64
    }
}

fn spread_weighted_target(ticket: u64, total_weight: u64) -> u64 {
    if total_weight <= 1 {
        return 0;
    }

    ticket.wrapping_mul(spread_weighted_stride(total_weight)) % total_weight
}

fn spread_weighted_stride(total_weight: u64) -> u64 {
    if total_weight <= 1 {
        return 1;
    }

    const PHI_NUMERATOR: u128 = 618_033_988_749_895;
    const PHI_DENOMINATOR: u128 = 1_000_000_000_000_000;

    let ideal =
        (((total_weight as u128) * PHI_NUMERATOR + (PHI_DENOMINATOR / 2)) / PHI_DENOMINATOR) as u64;
    let ideal = ideal.clamp(1, total_weight - 1);

    for delta in 0..total_weight {
        if let Some(candidate) = ideal.checked_sub(delta)
            && candidate > 0
            && gcd(candidate, total_weight) == 1
        {
            return candidate;
        }
        if let Some(candidate) = ideal.checked_add(delta)
            && candidate < total_weight
            && gcd(candidate, total_weight) == 1
        {
            return candidate;
        }
    }

    1
}

fn gcd(mut left: u64, mut right: u64) -> u64 {
    while right != 0 {
        let next = left % right;
        left = right;
        right = next;
    }
    left
}

fn backend_ref_is_routable(backend_ref: &BackendRef) -> bool {
    if backend_ref
        .metadata
        .get(BACKEND_REF_META_VALID)
        .is_some_and(|value| value == "false")
    {
        return false;
    }
    matches!(
        (backend_ref.group.as_str(), backend_ref.kind.as_str()),
        ("", "") | ("", "Service") | ("multicluster.x-k8s.io", "ServiceImport")
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Instant;

    #[test]
    fn collect_http_backend_candidates_preserves_backend_names() {
        let snapshot = Snapshot {
            backends: vec![BackendCluster {
                name: "orders:8080".to_string(),
                namespace: "default".to_string(),
                protocol: "HTTP".to_string(),
                endpoints: vec![BackendEndpoint {
                    address: "10.0.0.10".to_string(),
                    port: 8080,
                    healthy: true,
                }],
                wasm_plugin: None,
                ai_service: None,
                token_policy: None,
            }],
            ..Snapshot::default()
        };
        let refs = vec![BackendRef {
            namespace: "default".to_string(),
            name: "orders".to_string(),
            port: 8080,
            weight: 1,
            ..BackendRef::default()
        }];

        let (candidates, saw_invalid_refs, saw_unhealthy_backend) =
            snapshot.collect_http_backend_candidates(&refs, Instant::now());

        assert!(!saw_invalid_refs);
        assert!(!saw_unhealthy_backend);
        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].backend_name, "default/orders:8080");
    }

    #[test]
    fn visit_http_backend_candidates_preserves_order_and_status() {
        let snapshot = Snapshot {
            backends: vec![
                BackendCluster {
                    name: "users:8080".to_string(),
                    namespace: "default".to_string(),
                    protocol: "HTTP".to_string(),
                    endpoints: vec![BackendEndpoint {
                        address: "10.0.0.10".to_string(),
                        port: 8080,
                        healthy: true,
                    }],
                    wasm_plugin: None,
                    ai_service: None,
                    token_policy: None,
                },
                BackendCluster {
                    name: "orders:8081".to_string(),
                    namespace: "default".to_string(),
                    protocol: "HTTP".to_string(),
                    endpoints: vec![BackendEndpoint {
                        address: "10.0.0.11".to_string(),
                        port: 8081,
                        healthy: false,
                    }],
                    wasm_plugin: None,
                    ai_service: None,
                    token_policy: None,
                },
                BackendCluster {
                    name: "payments:8082".to_string(),
                    namespace: "default".to_string(),
                    protocol: "HTTP".to_string(),
                    endpoints: vec![BackendEndpoint {
                        address: "10.0.0.12".to_string(),
                        port: 8082,
                        healthy: true,
                    }],
                    wasm_plugin: None,
                    ai_service: None,
                    token_policy: None,
                },
            ],
            ..Snapshot::default()
        };
        let refs = vec![
            BackendRef {
                namespace: "default".to_string(),
                name: "users".to_string(),
                port: 8080,
                weight: 2,
                ..BackendRef::default()
            },
            BackendRef {
                namespace: "default".to_string(),
                name: "ignored-zero".to_string(),
                port: 8088,
                weight: 0,
                ..BackendRef::default()
            },
            BackendRef {
                namespace: "default".to_string(),
                name: "invalid".to_string(),
                port: 8089,
                weight: 1,
                metadata: BTreeMap::from([(
                    BACKEND_REF_META_VALID.to_string(),
                    "false".to_string(),
                )]),
                ..BackendRef::default()
            },
            BackendRef {
                namespace: "default".to_string(),
                name: "orders".to_string(),
                port: 8081,
                weight: 4,
                ..BackendRef::default()
            },
            BackendRef {
                namespace: "default".to_string(),
                name: "payments".to_string(),
                port: 8082,
                weight: 3,
                ..BackendRef::default()
            },
        ];

        let mut visited = Vec::new();
        let outcome = snapshot.visit_http_backend_candidates(&refs, Instant::now(), |candidate| {
            visited.push((
                candidate.backend_name.into_owned(),
                candidate.backend_ref.weight,
            ));
            true
        });

        assert_eq!(
            visited,
            vec![
                ("default/users:8080".to_string(), 2),
                ("default/payments:8082".to_string(), 3),
            ]
        );
        assert_eq!(outcome.candidate_count, 2);
        assert_eq!(outcome.total_weight, 5);
        assert!(outcome.saw_invalid_refs);
        assert!(outcome.saw_unhealthy_backend);
    }

    #[test]
    fn backend_policy_helpers_borrow_snapshot_values() {
        let snapshot = Snapshot {
            backend_policies: BTreeMap::from([(
                "default/users:8080".to_string(),
                BackendPolicy {
                    session_persistence: Some(SessionPersistence {
                        session_name: "ntgw-session".to_string(),
                        session_type: "Header".to_string(),
                        ..SessionPersistence::default()
                    }),
                    load_balancing: Some(LoadBalancingPolicy {
                        policy_type: "ConsistentHash".to_string(),
                        consistent_hash: Some(ConsistentHashPolicy {
                            key_type: "Header".to_string(),
                            header_name: "x-user-id".to_string(),
                        }),
                    }),
                    ..BackendPolicy::default()
                },
            )]),
            ..Snapshot::default()
        };
        let stored = snapshot.backend_policy("default/users:8080").unwrap();

        let session = snapshot
            .backend_session_persistence("default/users:8080")
            .unwrap();
        let load_balancing = snapshot
            .backend_load_balancing("default/users:8080")
            .unwrap();

        assert!(std::ptr::eq(
            session,
            stored.session_persistence.as_ref().unwrap()
        ));
        assert!(std::ptr::eq(
            load_balancing,
            stored.load_balancing.as_ref().unwrap()
        ));
    }

    #[test]
    fn visit_http_backend_candidates_borrows_indexed_backend_names() {
        let mut snapshot = Snapshot {
            backends: vec![BackendCluster {
                name: "users:8080".to_string(),
                namespace: "default".to_string(),
                protocol: "HTTP".to_string(),
                endpoints: vec![BackendEndpoint {
                    address: "10.0.0.10".to_string(),
                    port: 8080,
                    healthy: true,
                }],
                wasm_plugin: None,
                ai_service: None,
                token_policy: None,
            }],
            ..Snapshot::default()
        };
        snapshot.rebuild_runtime_indexes();
        let refs = vec![BackendRef {
            namespace: "default".to_string(),
            name: "users".to_string(),
            port: 8080,
            weight: 1,
            ..BackendRef::default()
        }];

        let mut saw_borrowed_backend_name = false;
        snapshot.visit_http_backend_candidates(&refs, Instant::now(), |candidate| {
            match candidate.backend_name {
                std::borrow::Cow::Borrowed(name) => {
                    saw_borrowed_backend_name = true;
                    assert!(std::ptr::eq(name, snapshot.backend_names[0].as_ref()));
                }
                std::borrow::Cow::Owned(_) => panic!("expected indexed backend name borrow"),
            }
            true
        });

        assert!(saw_borrowed_backend_name);
    }

    #[test]
    fn endpoint_rendezvous_hash_matches_string_port_parts() {
        for port in [0, 1, 80, 8080, u32::MAX] {
            let endpoint = BackendEndpoint {
                address: "10.0.0.10".to_string(),
                port,
                healthy: true,
            };
            let port_text = port.to_string();

            assert_eq!(
                rendezvous_hash_endpoint("hash-key", "default/orders:8080", &endpoint),
                rendezvous_hash(&[
                    "hash-key",
                    "default/orders:8080",
                    endpoint.address.as_str(),
                    port_text.as_str(),
                ])
            );
        }
    }
}
