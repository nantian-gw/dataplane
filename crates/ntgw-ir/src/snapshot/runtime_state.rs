use super::*;

impl Snapshot {
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
}
