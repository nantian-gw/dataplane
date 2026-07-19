use super::*;

impl Snapshot {
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
}
