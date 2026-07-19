use super::*;

impl Snapshot {
    pub(crate) fn select_first_healthy_backend(&self) -> Option<SelectedBackend> {
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

    pub(crate) fn select_listener_default_backend(
        &self,
        request: &RequestMeta,
    ) -> Option<SelectedBackend> {
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

    pub(crate) fn default_stream_backend(&self, listener: &Listener) -> Option<SelectedBackend> {
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
}
