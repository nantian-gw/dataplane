use super::{TrafficEdgeStat, TrafficNodeStat, TrafficState, merge_request_latency_histogram};

#[cfg(test)]
pub(super) struct ObservedEdge {
    pub(super) edge_id: String,
    pub(super) source: String,
    pub(super) target: String,
    pub(super) bytes_received: u64,
    pub(super) bytes_sent: u64,
}

pub(super) struct ObservedEdgeRef<'a> {
    pub(super) edge_id: &'a str,
    pub(super) source: &'a str,
    pub(super) target: &'a str,
    pub(super) bytes_received: u64,
    pub(super) bytes_sent: u64,
}

impl TrafficState {
    #[cfg(test)]
    pub(super) fn observe_node(
        &mut self,
        node_id: String,
        runtime_id: Option<u64>,
        bytes_received: u64,
        bytes_sent: u64,
        now: u128,
        limit: usize,
    ) {
        if limit == 0 {
            return;
        }

        if let Some(entry) = self.nodes.get_mut(node_id.as_str()) {
            observe_node_entry(entry, runtime_id, bytes_received, bytes_sent, now);
            return;
        }

        self.evict_oldest_node_if_needed(limit, node_id.as_str());
        let mut entry = TrafficNodeStat {
            node_id: node_id.clone(),
            runtime_id,
            ..TrafficNodeStat::default()
        };
        observe_node_entry(&mut entry, runtime_id, bytes_received, bytes_sent, now);
        self.nodes.insert(node_id, entry);
    }

    pub(super) fn observe_node_ref(
        &mut self,
        node_id: &str,
        runtime_id: Option<u64>,
        bytes_received: u64,
        bytes_sent: u64,
        now: u128,
        limit: usize,
    ) {
        if limit == 0 {
            return;
        }

        if let Some(entry) = self.nodes.get_mut(node_id) {
            observe_node_entry(entry, runtime_id, bytes_received, bytes_sent, now);
            return;
        }

        self.evict_oldest_node_if_needed(limit, node_id);
        let mut entry = TrafficNodeStat {
            node_id: node_id.to_string(),
            runtime_id,
            ..TrafficNodeStat::default()
        };
        observe_node_entry(&mut entry, runtime_id, bytes_received, bytes_sent, now);
        self.nodes.insert(node_id.to_string(), entry);
    }

    #[cfg(test)]
    pub(super) fn observe_edge(&mut self, edge: ObservedEdge, now: u128, limit: usize) {
        if limit == 0 {
            return;
        }

        if let Some(entry) = self.edges.get_mut(edge.edge_id.as_str()) {
            observe_edge_entry(entry, edge.bytes_received, edge.bytes_sent, now);
            return;
        }

        self.evict_oldest_edge_if_needed(limit, edge.edge_id.as_str());
        let mut entry = TrafficEdgeStat {
            edge_id: edge.edge_id.clone(),
            source: edge.source,
            target: edge.target,
            ..TrafficEdgeStat::default()
        };
        observe_edge_entry(&mut entry, edge.bytes_received, edge.bytes_sent, now);
        self.edges.insert(edge.edge_id, entry);
    }

    pub(super) fn observe_edge_ref(&mut self, edge: ObservedEdgeRef<'_>, now: u128, limit: usize) {
        if limit == 0 {
            return;
        }

        if let Some(entry) = self.edges.get_mut(edge.edge_id) {
            observe_edge_entry(entry, edge.bytes_received, edge.bytes_sent, now);
            return;
        }

        self.evict_oldest_edge_if_needed(limit, edge.edge_id);
        let mut entry = TrafficEdgeStat {
            edge_id: edge.edge_id.to_string(),
            source: edge.source.to_string(),
            target: edge.target.to_string(),
            ..TrafficEdgeStat::default()
        };
        observe_edge_entry(&mut entry, edge.bytes_received, edge.bytes_sent, now);
        self.edges.insert(edge.edge_id.to_string(), entry);
    }

    fn evict_oldest_node_if_needed(&mut self, limit: usize, retained_node_id: &str) {
        if self.nodes.len() < limit || self.nodes.contains_key(retained_node_id) {
            return;
        }

        let oldest_node_id = self
            .nodes
            .values()
            .min_by(|left, right| {
                left.last_seen_unix_ms
                    .cmp(&right.last_seen_unix_ms)
                    .then_with(|| left.node_id.cmp(&right.node_id))
            })
            .map(|node| node.node_id.clone());
        if let Some(oldest_node_id) = oldest_node_id {
            self.nodes.remove(oldest_node_id.as_str());
        }
    }

    fn evict_oldest_edge_if_needed(&mut self, limit: usize, retained_edge_id: &str) {
        if self.edges.len() < limit || self.edges.contains_key(retained_edge_id) {
            return;
        }

        let oldest_edge_id = self
            .edges
            .values()
            .min_by(|left, right| {
                left.last_seen_unix_ms
                    .cmp(&right.last_seen_unix_ms)
                    .then_with(|| left.edge_id.cmp(&right.edge_id))
            })
            .map(|edge| edge.edge_id.clone());
        if let Some(oldest_edge_id) = oldest_edge_id {
            self.edges.remove(oldest_edge_id.as_str());
        }
    }

    pub(super) fn merge_from(&mut self, other: &Self) {
        self.total_events = self.total_events.saturating_add(other.total_events);
        self.total_request_events = self
            .total_request_events
            .saturating_add(other.total_request_events);
        self.total_bytes_received = self
            .total_bytes_received
            .saturating_add(other.total_bytes_received);
        self.total_bytes_sent = self.total_bytes_sent.saturating_add(other.total_bytes_sent);
        self.total_latency_ms = self.total_latency_ms.saturating_add(other.total_latency_ms);
        self.max_latency_ms = self.max_latency_ms.max(other.max_latency_ms);
        for (labels, histogram) in &other.request_latency_ms_histograms {
            merge_request_latency_histogram(
                &mut self.request_latency_ms_histograms,
                labels,
                histogram,
            );
        }
        self.total_retried_events = self
            .total_retried_events
            .saturating_add(other.total_retried_events);
        self.total_retry_attempts = self
            .total_retry_attempts
            .saturating_add(other.total_retry_attempts);
        self.total_retried_success_events = self
            .total_retried_success_events
            .saturating_add(other.total_retried_success_events);
        self.total_upstream_pool_hits = self
            .total_upstream_pool_hits
            .saturating_add(other.total_upstream_pool_hits);
        self.total_upstream_pool_misses = self
            .total_upstream_pool_misses
            .saturating_add(other.total_upstream_pool_misses);
        self.total_upstream_peer_build_failures = self
            .total_upstream_peer_build_failures
            .saturating_add(other.total_upstream_peer_build_failures);
        self.total_upstream_connect_latency_observations = self
            .total_upstream_connect_latency_observations
            .saturating_add(other.total_upstream_connect_latency_observations);
        self.total_upstream_connect_latency_ms = self
            .total_upstream_connect_latency_ms
            .saturating_add(other.total_upstream_connect_latency_ms);
        self.max_upstream_connect_latency_ms = self
            .max_upstream_connect_latency_ms
            .max(other.max_upstream_connect_latency_ms);
        for (index, count) in other.upstream_connect_latency_ms_buckets.iter().enumerate() {
            self.upstream_connect_latency_ms_buckets[index] =
                self.upstream_connect_latency_ms_buckets[index].saturating_add(*count);
        }
        self.total_upstream_tls_handshake_failures = self
            .total_upstream_tls_handshake_failures
            .saturating_add(other.total_upstream_tls_handshake_failures);
        self.total_upstream_tls_handshake_failure_latency_observations = self
            .total_upstream_tls_handshake_failure_latency_observations
            .saturating_add(other.total_upstream_tls_handshake_failure_latency_observations);
        self.total_upstream_tls_handshake_failure_latency_ms = self
            .total_upstream_tls_handshake_failure_latency_ms
            .saturating_add(other.total_upstream_tls_handshake_failure_latency_ms);
        self.max_upstream_tls_handshake_failure_latency_ms = self
            .max_upstream_tls_handshake_failure_latency_ms
            .max(other.max_upstream_tls_handshake_failure_latency_ms);
        for (index, count) in other
            .upstream_tls_handshake_failure_latency_ms_buckets
            .iter()
            .enumerate()
        {
            self.upstream_tls_handshake_failure_latency_ms_buckets[index] = self
                .upstream_tls_handshake_failure_latency_ms_buckets[index]
                .saturating_add(*count);
        }
        self.status_1xx = self.status_1xx.saturating_add(other.status_1xx);
        self.status_2xx = self.status_2xx.saturating_add(other.status_2xx);
        self.status_3xx = self.status_3xx.saturating_add(other.status_3xx);
        self.status_4xx = self.status_4xx.saturating_add(other.status_4xx);
        self.status_5xx = self.status_5xx.saturating_add(other.status_5xx);
        self.status_other = self.status_other.saturating_add(other.status_other);
        self.normal_response_events = self
            .normal_response_events
            .saturating_add(other.normal_response_events);
        for (flag, events) in &other.response_flags {
            let entry = self.response_flags.entry(flag.clone()).or_default();
            *entry = entry.saturating_add(*events);
        }

        for node in other.nodes.values() {
            let entry = self
                .nodes
                .entry(node.node_id.clone())
                .or_insert_with(|| TrafficNodeStat {
                    node_id: node.node_id.clone(),
                    runtime_id: node.runtime_id,
                    ..TrafficNodeStat::default()
                });
            if entry.runtime_id.is_none() {
                entry.runtime_id = node.runtime_id;
            }
            entry.events = entry.events.saturating_add(node.events);
            entry.bytes_received = entry.bytes_received.saturating_add(node.bytes_received);
            entry.bytes_sent = entry.bytes_sent.saturating_add(node.bytes_sent);
            entry.last_seen_unix_ms = entry.last_seen_unix_ms.max(node.last_seen_unix_ms);
        }

        for edge in other.edges.values() {
            let entry = self
                .edges
                .entry(edge.edge_id.clone())
                .or_insert_with(|| TrafficEdgeStat {
                    edge_id: edge.edge_id.clone(),
                    source: edge.source.clone(),
                    target: edge.target.clone(),
                    ..TrafficEdgeStat::default()
                });
            entry.events = entry.events.saturating_add(edge.events);
            entry.bytes_received = entry.bytes_received.saturating_add(edge.bytes_received);
            entry.bytes_sent = entry.bytes_sent.saturating_add(edge.bytes_sent);
            entry.last_seen_unix_ms = entry.last_seen_unix_ms.max(edge.last_seen_unix_ms);
        }
    }
}

fn observe_node_entry(
    entry: &mut TrafficNodeStat,
    runtime_id: Option<u64>,
    bytes_received: u64,
    bytes_sent: u64,
    now: u128,
) {
    if entry.runtime_id.is_none() {
        entry.runtime_id = runtime_id;
    }
    entry.events += 1;
    entry.bytes_received += bytes_received;
    entry.bytes_sent += bytes_sent;
    entry.last_seen_unix_ms = now;
}

fn observe_edge_entry(
    entry: &mut TrafficEdgeStat,
    bytes_received: u64,
    bytes_sent: u64,
    now: u128,
) {
    entry.events += 1;
    entry.bytes_received += bytes_received;
    entry.bytes_sent += bytes_sent;
    entry.last_seen_unix_ms = now;
}
