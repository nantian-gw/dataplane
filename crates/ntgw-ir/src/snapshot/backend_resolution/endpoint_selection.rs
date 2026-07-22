use super::*;
use crate::http_fast_path::{CompiledHttpFastBackendRef, CompiledHttpFastBackendSelection};
use std::sync::Arc;
use std::time::Instant;

#[derive(Clone, Copy)]
pub(super) struct EndpointSelectionAvailability {
    pub(super) count: usize,
    pub(super) include_passive_ejected: bool,
}

#[derive(Clone, Copy)]
enum EndpointAvailability {
    Primary,
    PassiveLastResort,
    Unavailable,
}

impl Snapshot {
    pub(in crate::snapshot) fn select_cluster_endpoint(
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

    pub(super) fn select_cluster_endpoint_by_hash(
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

    pub(super) fn select_compiled_http_fast_endpoint(
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

    pub(super) fn eligible_endpoint_count(
        &self,
        cluster: &BackendCluster,
        backend_name: &str,
        now: Instant,
    ) -> usize {
        self.endpoint_selection_availability(cluster, backend_name, now)
            .count
    }

    pub(super) fn endpoint_selection_availability(
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

    pub(super) fn endpoint_matches_selection_availability(
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

    pub(in crate::snapshot) fn endpoint_is_available_at(
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
