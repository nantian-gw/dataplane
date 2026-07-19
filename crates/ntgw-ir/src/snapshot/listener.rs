use super::*;
use std::borrow::Cow;

impl Snapshot {
    pub(crate) fn rebuild_listener_indexes(&mut self) {
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
    ) -> Option<Cow<'a, mesh::ServiceFrontendRef>> {
        if self.runtime_indexes_ready {
            return self
                .service_frontend_index
                .get(listener.name.as_str())
                .map(Cow::Borrowed);
        }

        mesh::service_frontend(listener).map(Cow::Owned)
    }

    pub(crate) fn backend_namespace_for_service(
        &self,
        name: &str,
        port: u32,
    ) -> Option<Cow<'_, str>> {
        if name.is_empty() {
            return None;
        }

        if self.runtime_indexes_ready {
            return self
                .backend_service_index
                .unique_namespace(name, port)
                .map(Cow::Borrowed);
        }

        self.unique_backend_namespace_for_service(name, port)
            .map(Cow::Owned)
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
    ) -> Cow<'_, str> {
        if self.runtime_indexes_ready
            && let Some(backend_name) = self.backend_names.get(index)
        {
            return Cow::Borrowed(backend_name.as_ref());
        }

        Cow::Owned(backend_cluster_name(cluster))
    }
}
