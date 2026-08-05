use super::*;

mod backend_resolution;
mod helpers;

#[cfg(test)]
use self::helpers::rendezvous_hash;
use self::helpers::{
    backend_cluster_matches_ref, backend_cluster_name, backend_cluster_service_key,
    build_grpc_route_hostname_index, build_http_route_hostname_index,
    build_request_materialization_hints, build_route_attachment_listener_index,
    build_stream_listener_route_index, consistent_hash_key, listener_attaches_route,
    listener_frontend_client_certificate_requirement, rendezvous_hash_endpoint,
    route_kind_for_listener, route_kind_from_name, stream_listener_server_name_score,
    weighted_rendezvous_score,
};

const FRONTEND_VALIDATION_REJECT_MODE: &str = "RejectClientCertificate";
const FRONTEND_VALIDATION_ALLOW_INSECURE_FALLBACK_MODE: &str = "AllowInsecureFallback";

impl Snapshot {
    pub fn shared() -> SharedSnapshot {
        Arc::new(ArcSwap::from_pointee(Self::default()))
    }

    pub fn rebuild_runtime_indexes(&mut self) {
        self.compile_matchers();
        self.backend_names = self
            .backends
            .iter()
            .map(|cluster| Arc::<str>::from(backend_cluster_name(cluster)))
            .collect();
        self.backend_index = self
            .backend_names
            .iter()
            .enumerate()
            .map(|(index, backend_name)| (Arc::clone(backend_name), index))
            .collect();
        self.backend_service_index = BackendServiceIndex::with_capacity(self.backends.len());
        for (index, cluster) in self.backends.iter().enumerate() {
            let Some((name, port)) = backend_cluster_service_key(cluster.name.as_str()) else {
                continue;
            };
            self.backend_service_index
                .insert(cluster.namespace.as_str(), name, port, index);
        }
        self.secret_index = self
            .secrets
            .iter()
            .enumerate()
            .map(|(index, secret)| (format!("{}/{}", secret.namespace, secret.name), index))
            .collect();
        self.workload_namespace_index = self
            .workloads
            .iter()
            .map(|workload| (workload.ip.clone(), workload.namespace.clone()))
            .collect();
        self.runtime_ids = RuntimeIdIndex::from_snapshot(self);
        self.rebuild_listener_indexes();
        self.http_route_hostname_index = build_http_route_hostname_index(&self.http_routes);
        self.grpc_route_hostname_index = build_grpc_route_hostname_index(&self.grpc_routes);
        self.route_attachment_listener_index =
            build_route_attachment_listener_index(&self.listeners);
        self.request_materialization = build_request_materialization_hints(self);
        self.http_fast_path = crate::http_fast_path::HttpFastPathPlan::build(self);
        self.stream_listener_route_index =
            build_stream_listener_route_index(&self.listeners, &self.stream_routes);
        self.stream_fast_path = crate::stream_fast_path::StreamFastPathPlan::build(self);
        self.runtime_indexes_ready = true;
    }

    pub fn inherit_runtime_state_from(&mut self, previous: &Snapshot) {
        self.inherit_runtime_state_values_from(previous);
        self.rebuild_runtime_indexes();
    }

    pub fn inherit_runtime_state_values_from(&mut self, previous: &Snapshot) {
        self.selection_state = previous.selection_state.clone();
        self.endpoint_runtime = self.inherit_endpoint_runtime(previous);
    }

    pub fn endpoint_runtime_handle(&self, selected: &SelectedBackend) -> EndpointRuntimeHandle {
        self.endpoint_runtime
            .handle_for_backend(selected.backend_name.as_str(), &selected.backend)
    }

    pub fn record_endpoint_failure(&self, selected: &SelectedBackend) {
        self.record_endpoint_failure_at(selected, Instant::now());
    }

    pub(crate) fn record_endpoint_failure_at(&self, selected: &SelectedBackend, now: Instant) {
        self.endpoint_runtime.record_failure_at(
            selected.backend_name.as_str(),
            &selected.backend,
            now,
        );
    }

    pub fn record_endpoint_success(&self, selected: &SelectedBackend) {
        self.endpoint_runtime
            .record_success(selected.backend_name.as_str(), &selected.backend);
    }

    pub fn record_endpoint_active_probe_failure(
        &self,
        backend_name: &str,
        endpoint: &BackendEndpoint,
        unhealthy_threshold: u32,
    ) {
        self.endpoint_runtime.record_active_probe_failure(
            backend_name,
            endpoint,
            unhealthy_threshold,
        );
    }

    pub fn record_endpoint_active_probe_success(
        &self,
        backend_name: &str,
        endpoint: &BackendEndpoint,
    ) {
        self.endpoint_runtime
            .record_active_probe_success(backend_name, endpoint);
    }

    pub fn endpoint_runtime_snapshot(&self) -> EndpointRuntimeSnapshot {
        self.endpoint_runtime.snapshot(Instant::now())
    }

    pub fn selected_backend_runtime_ids(
        &self,
        selected: &SelectedBackend,
    ) -> SelectedBackendRuntimeIds {
        self.runtime_ids.selected_backend(self, selected)
    }

    pub fn runtime_resource_ref(&self, runtime_id: RuntimeId) -> Option<RuntimeResourceRef> {
        self.runtime_ids.resource_ref(runtime_id)
    }

    pub fn listener_runtime_id(&self, listener_name: &str) -> Option<RuntimeId> {
        self.runtime_ids.listener(listener_name)
    }

    pub fn http_route_runtime_id(&self, namespace: &str, name: &str) -> Option<RuntimeId> {
        self.runtime_ids.http_route(namespace, name)
    }

    pub fn grpc_route_runtime_id(&self, namespace: &str, name: &str) -> Option<RuntimeId> {
        self.runtime_ids.grpc_route(namespace, name)
    }

    pub fn stream_route_runtime_id(
        &self,
        kind: &str,
        namespace: &str,
        name: &str,
    ) -> Option<RuntimeId> {
        self.runtime_ids.stream_route(kind, namespace, name)
    }

    pub fn http_rule_runtime_id(
        &self,
        namespace: &str,
        name: &str,
        rule_index: usize,
    ) -> Option<RuntimeId> {
        self.runtime_ids.http_rule(namespace, name, rule_index)
    }

    pub fn grpc_rule_runtime_id(
        &self,
        namespace: &str,
        name: &str,
        rule_index: usize,
    ) -> Option<RuntimeId> {
        self.runtime_ids.grpc_rule(namespace, name, rule_index)
    }

    pub fn stream_rule_runtime_id(
        &self,
        kind: &str,
        namespace: &str,
        name: &str,
        rule_index: usize,
    ) -> Option<RuntimeId> {
        self.runtime_ids
            .stream_rule(kind, namespace, name, rule_index)
    }

    pub fn backend_runtime_id(&self, backend_name: &str) -> Option<RuntimeId> {
        self.runtime_ids.backend(backend_name)
    }

    pub fn endpoint_runtime_id(
        &self,
        backend_name: &str,
        endpoint: &BackendEndpoint,
    ) -> Option<RuntimeId> {
        self.runtime_ids.endpoint(backend_name, endpoint)
    }

    fn compile_matchers(&mut self) {
        for route in &mut self.http_routes {
            for rule in &mut route.rules {
                for matcher in &mut rule.matches {
                    matcher.compile_runtime_state();
                }
            }
        }

        for route in &mut self.grpc_routes {
            for rule in &mut route.rules {
                for matcher in &mut rule.matches {
                    matcher.compile_runtime_state();
                }
            }
        }
    }

    pub fn backend_policy(&self, backend_name: &str) -> Option<&BackendPolicy> {
        self.backend_policies.get(backend_name)
    }

    pub fn backend_protocol(&self, backend_name: &str) -> Option<&str> {
        self.backend_cluster_by_name(backend_name)
            .map(|cluster| cluster.protocol.as_str())
    }

    pub fn secret_material(&self, namespace: &str, name: &str) -> Option<&SecretMaterial> {
        if self.runtime_indexes_ready {
            let secret_name = format!("{namespace}/{name}");
            if let Some(secret) = self
                .secret_index
                .get(secret_name.as_str())
                .and_then(|index| self.secrets.get(*index))
            {
                return Some(secret);
            }
        }

        self.secrets
            .iter()
            .find(|secret| secret.namespace == namespace && secret.name == name)
    }

    pub fn select_backend(&self, request: &RequestMeta) -> Option<SelectedBackend> {
        self.select_backend_with_session_resolver(request, |_| None)
    }

    pub fn select_backend_with_session_resolver<F>(
        &self,
        request: &RequestMeta,
        session_resolver: F,
    ) -> Option<SelectedBackend>
    where
        F: Fn(&SessionPersistence) -> Option<PersistentSessionTarget>,
    {
        let targets_mesh_service_frontend = self.request_targets_mesh_service_frontend(request)
            || if is_grpc_request(request) {
                http_selection::matches_service_frontend_grpc_listener(self, request)
            } else {
                http_selection::matches_service_frontend_http_listener(self, request)
            };

        if is_grpc_request(request)
            && let Some(selected) =
                self.select_grpc_backend_with_session_resolver(request, &session_resolver)
        {
            return Some(selected);
        }

        let allow_mesh_service_fallback = if is_grpc_request(request) {
            !http_selection::has_service_frontend_grpc_route_candidate(self, request)
        } else {
            !http_selection::has_service_frontend_http_route_candidate(self, request)
        };

        self.select_http_backend_with_session_resolver(request, &session_resolver)
            .or_else(|| {
                allow_mesh_service_fallback
                    .then(|| self.select_listener_default_backend(request))
                    .flatten()
            })
            .or_else(|| {
                (!targets_mesh_service_frontend && !self.has_l7_routes())
                    .then(|| self.select_first_healthy_backend())
                    .flatten()
            })
    }

    pub fn select_http_backend(&self, request: &RequestMeta) -> Option<SelectedBackend> {
        self.select_http_backend_with_session_resolver(request, |_| None)
    }

    pub fn select_http_backend_with_session_resolver<F>(
        &self,
        request: &RequestMeta,
        session_resolver: F,
    ) -> Option<SelectedBackend>
    where
        F: Fn(&SessionPersistence) -> Option<PersistentSessionTarget>,
    {
        self.select_http_route_with_session_resolver(request, session_resolver)
            .and_then(SelectedHttpRoute::into_backend)
    }

    pub fn select_http_route(&self, request: &RequestMeta) -> Option<SelectedHttpRoute> {
        self.select_http_route_with_session_resolver(request, |_| None)
    }

    pub fn select_http_route_with_session_resolver<F>(
        &self,
        request: &RequestMeta,
        session_resolver: F,
    ) -> Option<SelectedHttpRoute>
    where
        F: Fn(&SessionPersistence) -> Option<PersistentSessionTarget>,
    {
        http_selection::select_http_route(self, request, &session_resolver)
    }

    pub fn select_http_fast_path(
        &self,
        request: HttpFastPathRequest<'_>,
    ) -> Option<CompiledSelectedHttpBackend> {
        self.http_fast_path.select(self, request)
    }

    pub fn select_grpc_backend(&self, request: &RequestMeta) -> Option<SelectedBackend> {
        self.select_grpc_backend_with_session_resolver(request, |_| None)
    }

    pub fn select_grpc_backend_with_session_resolver<F>(
        &self,
        request: &RequestMeta,
        session_resolver: F,
    ) -> Option<SelectedBackend>
    where
        F: Fn(&SessionPersistence) -> Option<PersistentSessionTarget>,
    {
        http_selection::select_grpc_backend(self, request, &session_resolver)
    }

    pub fn https_request_is_misdirected(
        &self,
        request: &RequestMeta,
        server_name: Option<&str>,
    ) -> bool {
        http_selection::https_request_is_misdirected(self, request, server_name)
    }

    pub fn select_stream_backend(
        &self,
        listener_name: &str,
        server_name: Option<&str>,
    ) -> Option<SelectedBackend> {
        let listener = self.listener_by_name(listener_name)?;
        stream_listener_server_name_score(listener, server_name)?;

        let expected_kind = route_kind_for_listener(&listener.protocol)?;
        self.select_stream_backend_for_listener(listener, expected_kind, server_name, None, true)
    }

    pub fn select_tls_stream_backend(
        &self,
        listener_name: &str,
        server_name: Option<&str>,
        mode: TlsRouteMode,
    ) -> Option<SelectedBackend> {
        let listener = self.listener_by_name(listener_name)?;
        stream_listener_server_name_score(listener, server_name)?;

        let expected_kind = route_kind_for_listener(&listener.protocol)?;
        if expected_kind != RouteKind::Tls {
            return None;
        }
        self.select_stream_backend_for_listener(
            listener,
            expected_kind,
            server_name,
            Some(mode),
            false,
        )
    }

    pub fn select_stream_backend_from_listener_set(
        &self,
        listener_names: &[String],
        server_name: Option<&str>,
    ) -> Option<SelectedBackend> {
        let mut selected = None;
        self.visit_best_stream_listeners(listener_names, server_name, |listener| {
            let Some(expected_kind) = route_kind_for_listener(&listener.protocol) else {
                return true;
            };
            if let Some(candidate) = self.select_stream_backend_for_listener(
                listener,
                expected_kind,
                server_name,
                None,
                true,
            ) {
                selected = Some(candidate);
                return false;
            }
            true
        });

        selected
    }

    pub(crate) fn visit_best_stream_listeners<'a>(
        &'a self,
        listener_names: &[String],
        server_name: Option<&str>,
        mut visit: impl FnMut(&'a Listener) -> bool,
    ) -> bool {
        let mut best_score = None;
        for listener_name in listener_names {
            let Some(listener) = self.listener_by_name(listener_name) else {
                continue;
            };
            let Some(score) = stream_listener_server_name_score(listener, server_name) else {
                continue;
            };
            if best_score.is_none_or(|current| score > current) {
                best_score = Some(score);
            }
        }

        let Some(best_score) = best_score else {
            return true;
        };
        for listener_name in listener_names {
            let Some(listener) = self.listener_by_name(listener_name) else {
                continue;
            };
            if stream_listener_server_name_score(listener, server_name) == Some(best_score)
                && !visit(listener)
            {
                return false;
            }
        }
        true
    }

    fn select_stream_backend_for_listener(
        &self,
        listener: &Listener,
        expected_kind: RouteKind,
        server_name: Option<&str>,
        tls_mode: Option<TlsRouteMode>,
        allow_default_backend: bool,
    ) -> Option<SelectedBackend> {
        if route_kind_for_listener(&listener.protocol) != Some(expected_kind) {
            return None;
        }
        if self.runtime_indexes_ready {
            return self
                .stream_fast_path
                .select(self, listener, expected_kind, server_name, tls_mode)
                .or_else(|| {
                    allow_default_backend
                        .then(|| self.default_stream_backend(listener))
                        .flatten()
                });
        }

        let mut best: Option<(SelectedBackend, StreamMatchScore)> = None;

        self.visit_stream_route_candidates(listener, &expected_kind, |route| {
            for (rule_index, rule) in route.rules.iter().enumerate() {
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
                    .is_some_and(|(_, current_score)| score <= *current_score)
                {
                    continue;
                }

                if let Some(selected) = self.resolve_backend_refs(&rule.backend_refs) {
                    best = Some((
                        SelectedBackend {
                            route_policy: None,
                            route_kind: expected_kind,
                            route_name: route.name.clone(),
                            route_namespace: route.namespace.clone(),
                            rule_index: Some(rule_index),
                            route_annotations: route.annotations.clone(),
                            listener_name: listener.name.clone(),
                            listener_protocol: listener.protocol.clone(),
                            backend: selected.0,
                            backend_name: selected.1,
                            filters: Vec::new(),
                            matched_http_path: None,
                            timeouts: None,
                            retry: None,
                            session_persistence: None,
                            backend_tls: None,
                        },
                        score,
                    ));
                }
            }
            true
        });

        if let Some((selected, _)) = best {
            return Some(selected);
        }

        allow_default_backend
            .then(|| self.default_stream_backend(listener))
            .flatten()
    }

    fn rebuild_listener_indexes(&mut self) {
        self.listener_name_index.clear();
        self.http_listener_indices.clear();
        self.grpc_listener_indices.clear();
        self.http_listener_port_index.clear();
        self.grpc_listener_port_index.clear();
        self.frontend_client_certificate_index.clear();
        self.service_frontend_index.clear();
        self.service_frontend_listener_port_index.clear();
        self.service_frontend_attachment_index.clear();

        for (index, listener) in self.listeners.iter().enumerate() {
            self.listener_name_index
                .entry(listener.name.clone())
                .or_insert(index);
            if let Some(frontend) = mesh::service_frontend(listener) {
                self.service_frontend_listener_port_index
                    .entry(listener.port)
                    .or_default()
                    .push(index);
                if !listener.attached_routes.is_empty() {
                    self.service_frontend_attachment_index
                        .entry(frontend.namespace.clone())
                        .or_default()
                        .insert(frontend.name.clone());
                }
                self.service_frontend_index
                    .insert(listener.name.clone(), frontend);
            }
            let requirement = listener_frontend_client_certificate_requirement(listener);
            if requirement != FrontendClientCertificateRequirement::None {
                self.frontend_client_certificate_index
                    .insert(listener.name.clone(), requirement);
            }
            if http_selection::is_http_listener(&listener.protocol) {
                self.http_listener_indices.push(index);
                self.http_listener_port_index
                    .entry(listener.port)
                    .or_default()
                    .push(index);
            }
            if http_selection::is_grpc_listener(&listener.protocol) {
                self.grpc_listener_indices.push(index);
                self.grpc_listener_port_index
                    .entry(listener.port)
                    .or_default()
                    .push(index);
            }
        }
    }

    pub(crate) fn listener_by_name(&self, listener_name: &str) -> Option<&Listener> {
        if self.runtime_indexes_ready {
            let index = self.listener_name_index.get(listener_name)?;
            return self.listeners.get(*index);
        }

        self.listeners
            .iter()
            .find(|listener| listener.name == listener_name)
    }

    pub fn listener_requires_frontend_client_certificate_close(
        &self,
        listener_name: &str,
        client_certificate_present: bool,
    ) -> bool {
        self.frontend_client_certificate_requirement(listener_name)
            .closes_connection_without_valid_client_certificate(client_certificate_present)
    }

    pub fn frontend_client_certificate_requirement(
        &self,
        listener_name: &str,
    ) -> FrontendClientCertificateRequirement {
        if listener_name.is_empty() {
            return FrontendClientCertificateRequirement::None;
        }
        self.frontend_client_certificate_index
            .get(listener_name)
            .copied()
            .unwrap_or_default()
    }

    #[cfg(test)]
    pub(crate) fn service_frontend_for_listener_name(
        &self,
        listener_name: &str,
    ) -> Option<mesh::ServiceFrontendRef> {
        if listener_name.is_empty() {
            return None;
        }
        if self.runtime_indexes_ready {
            return self.service_frontend_index.get(listener_name).cloned();
        }

        self.listeners
            .iter()
            .find(|listener| listener.name == listener_name)
            .and_then(mesh::service_frontend)
    }

    pub(crate) fn service_frontend_for_listener<'a>(
        &'a self,
        listener: &'a Listener,
    ) -> Option<std::borrow::Cow<'a, mesh::ServiceFrontendRef>> {
        if self.runtime_indexes_ready {
            return self
                .service_frontend_index
                .get(listener.name.as_str())
                .map(std::borrow::Cow::Borrowed);
        }

        mesh::service_frontend(listener).map(std::borrow::Cow::Owned)
    }

    pub(crate) fn backend_namespace_for_service(
        &self,
        name: &str,
        port: u32,
    ) -> Option<std::borrow::Cow<'_, str>> {
        if name.is_empty() {
            return None;
        }

        if self.runtime_indexes_ready {
            return self
                .backend_service_index
                .unique_namespace(name, port)
                .map(std::borrow::Cow::Borrowed);
        }

        self.unique_backend_namespace_for_service(name, port)
            .map(std::borrow::Cow::Owned)
    }

    pub(crate) fn backend_cluster_for_service(
        &self,
        namespace: &str,
        name: &str,
        port: u32,
    ) -> Option<&BackendCluster> {
        self.backend_cluster_for_service_index(namespace, name, port)
            .map(|(_, cluster)| cluster)
    }

    pub(crate) fn backend_cluster_for_service_index(
        &self,
        namespace: &str,
        name: &str,
        port: u32,
    ) -> Option<(usize, &BackendCluster)> {
        if self.runtime_indexes_ready {
            let index = self
                .backend_service_index
                .index_for(namespace, name, port)?;
            return self.backends.get(index).map(|cluster| (index, cluster));
        }

        self.backends
            .iter()
            .enumerate()
            .find(|(_, cluster)| backend_cluster_matches_ref(cluster, namespace, name, port))
    }

    pub(crate) fn backend_name_for_index(
        &self,
        index: usize,
        cluster: &BackendCluster,
    ) -> std::borrow::Cow<'_, str> {
        if self.runtime_indexes_ready
            && let Some(backend_name) = self.backend_names.get(index)
        {
            return std::borrow::Cow::Borrowed(backend_name.as_ref());
        }

        std::borrow::Cow::Owned(backend_cluster_name(cluster))
    }

    pub(crate) fn visit_stream_route_candidates<'a>(
        &'a self,
        listener: &'a Listener,
        expected_kind: &RouteKind,
        mut visit: impl FnMut(&'a StreamRoute) -> bool,
    ) -> bool {
        if self.runtime_indexes_ready {
            let Some(indices) = self.stream_listener_route_index.get(listener.name.as_str()) else {
                return true;
            };

            for index in indices {
                let Some(route) = self.stream_routes.get(*index) else {
                    continue;
                };
                if route_kind_from_name(&route.kind) == Some(*expected_kind) && !visit(route) {
                    return false;
                }
            }
            return true;
        }

        for route in &self.stream_routes {
            if route_kind_from_name(&route.kind) == Some(*expected_kind)
                && listener_attaches_route(listener, route)
                && !visit(route)
            {
                return false;
            }
        }
        true
    }

    pub fn select_request_mirror(&self, context: &RequestMirrorContext) -> Option<SelectedBackend> {
        let mut selected = None;
        self.visit_request_mirrors(context, |mirror| {
            selected = Some(mirror);
            false
        });
        selected
    }

    pub fn select_request_mirrors(&self, context: &RequestMirrorContext) -> Vec<SelectedBackend> {
        let mut mirrors = Vec::new();
        self.visit_request_mirrors(context, |mirror| {
            mirrors.push(mirror);
            true
        });
        mirrors
    }

    pub(crate) fn visit_request_mirrors(
        &self,
        context: &RequestMirrorContext,
        mut visit: impl FnMut(SelectedBackend) -> bool,
    ) -> bool {
        let mut mirrored_filters = None;
        for filter in &context.filters {
            let Some(filter) = filter.request_mirror.as_ref() else {
                continue;
            };
            if !self.should_mirror(filter) {
                continue;
            }
            let Some((backend, backend_name)) = self.select_backend_ref(&filter.backend_ref) else {
                continue;
            };

            let filters = mirrored_filters
                .get_or_insert_with(|| filters_without_request_mirror(&context.filters));
            let mirror = SelectedBackend {
                route_policy: None,
                route_kind: context.route_kind,
                route_name: context.route_name.clone(),
                route_namespace: context.route_namespace.clone(),
                rule_index: context.rule_index,
                route_annotations: BTreeMap::new(),
                listener_name: String::new(),
                listener_protocol: String::new(),
                backend,
                backend_name,
                filters: filters.clone(),
                matched_http_path: context.matched_http_path.clone(),
                timeouts: context.timeouts.clone(),
                retry: None,
                session_persistence: None,
                backend_tls: context.backend_tls.clone(),
            };
            if !visit(mirror) {
                return false;
            }
        }
        true
    }

    fn select_first_healthy_backend(&self) -> Option<SelectedBackend> {
        let now = Instant::now();
        for cluster in &self.backends {
            let backend_name = backend_cluster_name(cluster);
            if let Some(endpoint) = cluster
                .endpoints
                .iter()
                .find(|item| self.endpoint_is_available_at(backend_name.as_str(), item, now))
            {
                return Some(SelectedBackend {
                    route_policy: None,
                    route_kind: RouteKind::Http,
                    route_name: String::new(),
                    route_namespace: String::new(),
                    rule_index: None,
                    route_annotations: BTreeMap::new(),
                    listener_name: String::new(),
                    listener_protocol: String::new(),
                    backend: endpoint.clone(),
                    backend_name,
                    filters: Vec::new(),
                    matched_http_path: None,
                    timeouts: None,
                    retry: None,
                    session_persistence: None,
                    backend_tls: None,
                });
            }
        }

        None
    }

    fn select_listener_default_backend(&self, request: &RequestMeta) -> Option<SelectedBackend> {
        if self.runtime_indexes_ready
            && let Some(selected) = self.select_listener_default_backend_from_indices(request)
        {
            return Some(selected);
        }

        self.select_listener_default_backend_slow_path(request)
    }

    fn service_frontend_has_attached_routes(&self, frontend: &mesh::ServiceFrontendRef) -> bool {
        if self.runtime_indexes_ready {
            return self
                .service_frontend_attachment_index
                .get(frontend.namespace.as_str())
                .is_some_and(|names| names.contains(frontend.name.as_str()));
        }

        self.listeners.iter().any(|listener| {
            !listener.attached_routes.is_empty()
                && self
                    .service_frontend_for_listener(listener)
                    .is_some_and(|candidate| {
                        candidate.namespace == frontend.namespace && candidate.name == frontend.name
                    })
        })
    }

    fn select_listener_default_backend_from_indices(
        &self,
        request: &RequestMeta,
    ) -> Option<SelectedBackend> {
        let indices = self
            .service_frontend_listener_port_index
            .get(&request.port)?;

        for index in indices {
            let listener = self.listeners.get(*index)?;
            let frontend = self.service_frontend_for_listener(listener)?;
            if listener.attached_routes.is_empty()
                && !self.service_frontend_has_attached_routes(&frontend)
            {
                continue;
            }

            let route_kind = match listener.protocol.as_str() {
                "LISTENER_PROTOCOL_GRPC" | "GRPC" => RouteKind::Grpc,
                _ => RouteKind::Http,
            };
            let backend =
                self.default_service_backend(&frontend.namespace, &frontend.name, frontend.port)?;

            return Some(SelectedBackend {
                route_policy: None,
                route_kind,
                route_name: String::new(),
                route_namespace: frontend.namespace.clone(),
                rule_index: None,
                route_annotations: BTreeMap::new(),
                listener_name: listener.name.clone(),
                listener_protocol: listener.protocol.clone(),
                backend: backend.0,
                backend_name: backend.1,
                filters: Vec::new(),
                matched_http_path: None,
                timeouts: None,
                retry: None,
                session_persistence: None,
                backend_tls: listener.backend_tls.clone(),
            });
        }

        None
    }

    fn select_listener_default_backend_slow_path(
        &self,
        request: &RequestMeta,
    ) -> Option<SelectedBackend> {
        let (listener, frontend) = self.listeners.iter().find_map(|listener| {
            (listener.port == request.port)
                .then(|| {
                    self.service_frontend_for_listener(listener)
                        .map(|frontend| (listener, frontend))
                })
                .flatten()
        })?;
        if listener.attached_routes.is_empty()
            && !self.service_frontend_has_attached_routes(&frontend)
        {
            return None;
        }
        let route_kind = match listener.protocol.as_str() {
            "LISTENER_PROTOCOL_GRPC" | "GRPC" => RouteKind::Grpc,
            _ => RouteKind::Http,
        };
        let backend =
            self.default_service_backend(&frontend.namespace, &frontend.name, frontend.port)?;

        Some(SelectedBackend {
            route_policy: None,
            route_kind,
            route_name: String::new(),
            route_namespace: frontend.namespace.clone(),
            rule_index: None,
            route_annotations: BTreeMap::new(),
            listener_name: listener.name.clone(),
            listener_protocol: listener.protocol.clone(),
            backend: backend.0,
            backend_name: backend.1,
            filters: Vec::new(),
            matched_http_path: None,
            timeouts: None,
            retry: None,
            session_persistence: None,
            backend_tls: listener.backend_tls.clone(),
        })
    }

    fn default_stream_backend(&self, listener: &Listener) -> Option<SelectedBackend> {
        let frontend = self.service_frontend_for_listener(listener)?;
        let route_kind = route_kind_for_listener(&listener.protocol)?;
        let backend =
            self.default_service_backend(&frontend.namespace, &frontend.name, frontend.port)?;

        Some(SelectedBackend {
            route_policy: None,
            route_kind,
            route_name: String::new(),
            route_namespace: frontend.namespace.clone(),
            rule_index: None,
            route_annotations: BTreeMap::new(),
            listener_name: listener.name.clone(),
            listener_protocol: listener.protocol.clone(),
            backend: backend.0,
            backend_name: backend.1,
            filters: Vec::new(),
            matched_http_path: None,
            timeouts: None,
            retry: None,
            session_persistence: None,
            backend_tls: None,
        })
    }

    fn default_service_backend(
        &self,
        namespace: &str,
        name: &str,
        port: u32,
    ) -> Option<(BackendEndpoint, String)> {
        self.select_service_backend(namespace, name, port)
    }

    fn select_service_backend(
        &self,
        namespace: &str,
        name: &str,
        port: u32,
    ) -> Option<(BackendEndpoint, String)> {
        let cluster = self.backend_cluster_for_service(namespace, name, port)?;
        let backend_name = backend_cluster_name(cluster);
        self.select_cluster_endpoint(cluster, backend_name.as_str())
            .map(|endpoint| (endpoint, backend_name))
    }

    fn request_targets_mesh_service_frontend(&self, request: &RequestMeta) -> bool {
        let Some(frontend) = self.request_service_frontend_ref(request) else {
            return false;
        };

        self.backend_cluster_for_service(
            frontend.namespace.as_str(),
            frontend.name.as_str(),
            frontend.port,
        )
        .is_some()
    }

    fn request_service_frontend_ref(
        &self,
        request: &RequestMeta,
    ) -> Option<mesh::ServiceFrontendRef> {
        let port = request.port;
        if port == 0 {
            return None;
        }

        let host = normalize_host_ref(request.host.as_deref()?);
        let mut parts = host.split('.').filter(|part| !part.is_empty());
        let name = parts.next()?;
        let namespace = match (parts.next(), parts.next()) {
            (None, None) => self
                .source_namespace(request)
                .map(str::to_string)
                .or_else(|| {
                    self.backend_namespace_for_service(name, port)
                        .map(std::borrow::Cow::into_owned)
                })?,
            (Some(namespace), None) => namespace.to_string(),
            (Some(namespace), Some("svc")) => namespace.to_string(),
            _ => return None,
        };

        Some(mesh::ServiceFrontendRef {
            namespace,
            name: name.to_string(),
            port,
        })
    }

    fn unique_backend_namespace_for_service(&self, name: &str, port: u32) -> Option<String> {
        let mut matched_namespace: Option<&str> = None;

        for cluster in &self.backends {
            let Some((cluster_name, cluster_port)) =
                backend_cluster_service_key(cluster.name.as_str())
            else {
                continue;
            };
            if cluster_name != name || cluster_port != port {
                continue;
            }
            match matched_namespace {
                Some(namespace) if namespace != cluster.namespace => return None,
                Some(_) => continue,
                None => matched_namespace = Some(cluster.namespace.as_str()),
            }
        }

        matched_namespace.map(str::to_string)
    }

    pub(crate) fn source_namespace(&self, request: &RequestMeta) -> Option<&str> {
        let source_ip = request.source_ip.as_deref()?;
        if self.runtime_indexes_ready
            && let Some(namespace) = self.workload_namespace_index.get(source_ip)
        {
            return Some(namespace.as_str());
        }

        self.workloads
            .iter()
            .find(|workload| workload.ip == source_ip)
            .map(|workload| workload.namespace.as_str())
    }

    fn has_l7_routes(&self) -> bool {
        !self.http_routes.is_empty() || !self.grpc_routes.is_empty()
    }
}
