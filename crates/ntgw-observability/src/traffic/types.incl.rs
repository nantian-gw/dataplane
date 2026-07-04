pub fn upstream_connect_latency_ms_bucket_index(latency_ms: u64) -> usize {
    UPSTREAM_CONNECT_LATENCY_MS_BUCKET_BOUNDS
        .iter()
        .position(|bound| latency_ms <= *bound)
        .unwrap_or(UPSTREAM_CONNECT_LATENCY_MS_BUCKET_BOUNDS.len())
}

pub fn traffic_latency_ms_bucket_index(latency_ms: u64) -> usize {
    TRAFFIC_LATENCY_MS_BUCKET_BOUNDS
        .iter()
        .position(|bound| latency_ms <= *bound)
        .unwrap_or(TRAFFIC_LATENCY_MS_BUCKET_BOUNDS.len())
}

#[derive(Debug, Clone, Default)]
pub struct TrafficObservation {
    pub listener_name: String,
    pub protocol: String,
    pub route_namespace: String,
    pub route_name: String,
    pub route_kind: String,
    pub backend_name: String,
    pub status: Option<u16>,
    pub latency_ms: u64,
    pub bytes_received: u64,
    pub bytes_sent: u64,
    pub retry_attempts: u32,
    pub retried_success: bool,
    pub upstream_pool_hits: u32,
    pub upstream_pool_misses: u32,
    pub upstream_peer_build_failures: u32,
    pub upstream_connect_latency_ms: u64,
    pub upstream_connect_latency_ms_max: u64,
    pub upstream_connect_latency_ms_buckets: [u32; UPSTREAM_CONNECT_LATENCY_MS_BUCKET_COUNT],
    pub response_flags: String,
    pub runtime_ids: TrafficRuntimeIds,
}

#[derive(Debug, Clone, Copy)]
pub struct TrafficObservationRef<'a> {
    pub listener_name: &'a str,
    pub protocol: &'a str,
    pub route_namespace: &'a str,
    pub route_name: &'a str,
    pub route_kind: &'a str,
    pub backend_name: &'a str,
    pub status: Option<u16>,
    pub latency_ms: u64,
    pub bytes_received: u64,
    pub bytes_sent: u64,
    pub retry_attempts: u32,
    pub retried_success: bool,
    pub upstream_pool_hits: u32,
    pub upstream_pool_misses: u32,
    pub upstream_peer_build_failures: u32,
    pub upstream_connect_latency_ms: u64,
    pub upstream_connect_latency_ms_max: u64,
    pub upstream_connect_latency_ms_buckets: &'a [u32; UPSTREAM_CONNECT_LATENCY_MS_BUCKET_COUNT],
    pub response_flags: &'a str,
    pub runtime_ids: TrafficRuntimeIds,
}

impl<'a> TrafficObservationRef<'a> {
    pub fn to_owned(&self) -> TrafficObservation {
        TrafficObservation {
            listener_name: self.listener_name.to_string(),
            protocol: self.protocol.to_string(),
            route_namespace: self.route_namespace.to_string(),
            route_name: self.route_name.to_string(),
            route_kind: self.route_kind.to_string(),
            backend_name: self.backend_name.to_string(),
            status: self.status,
            latency_ms: self.latency_ms,
            bytes_received: self.bytes_received,
            bytes_sent: self.bytes_sent,
            retry_attempts: self.retry_attempts,
            retried_success: self.retried_success,
            upstream_pool_hits: self.upstream_pool_hits,
            upstream_pool_misses: self.upstream_pool_misses,
            upstream_peer_build_failures: self.upstream_peer_build_failures,
            upstream_connect_latency_ms: self.upstream_connect_latency_ms,
            upstream_connect_latency_ms_max: self.upstream_connect_latency_ms_max,
            upstream_connect_latency_ms_buckets: *self.upstream_connect_latency_ms_buckets,
            response_flags: self.response_flags.to_string(),
            runtime_ids: self.runtime_ids,
        }
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct TrafficTopology {
    pub route_kind: Cow<'static, str>,
    pub shard_key: u64,
    pub listener_node_id: String,
    pub route_node_id: String,
    pub backend_node_id: Option<String>,
    pub endpoint_set_node_id: Option<String>,
    pub dataplane_to_listener_edge_id: String,
    pub listener_to_route_edge_id: String,
    pub route_to_backend_edge_id: Option<String>,
    pub backend_to_endpoint_set_edge_id: Option<String>,
}

#[derive(Debug, Clone, Copy)]
pub struct TrafficTopologyRef<'a> {
    pub route_kind: &'a str,
    pub shard_key: u64,
    pub listener_node_id: &'a str,
    pub route_node_id: &'a str,
    pub backend_node_id: Option<&'a str>,
    pub endpoint_set_node_id: Option<&'a str>,
    pub dataplane_to_listener_edge_id: &'a str,
    pub listener_to_route_edge_id: &'a str,
    pub route_to_backend_edge_id: Option<&'a str>,
    pub backend_to_endpoint_set_edge_id: Option<&'a str>,
}

impl TrafficTopology {
    pub fn from_parts(
        listener_name: &str,
        route_kind: &str,
        route_namespace: &str,
        route_name: &str,
        backend_name: &str,
    ) -> Self {
        let route_kind = canonical_route_kind_ref(route_kind);
        let listener_id = listener_node_id(listener_name);
        let route_id = route_node_id(route_kind.as_ref(), route_namespace, route_name);
        let backend = parse_backend_name_ref(backend_name);
        let endpoint_set_id = backend
            .as_ref()
            .map(|backend| endpoint_set_node_id(backend.namespace, backend.name, backend.port));
        let backend_id = backend
            .as_ref()
            .map(|backend| backend_node_id(backend.namespace, backend.name, backend.port));
        let route_to_backend_edge_id = backend_id
            .as_ref()
            .map(|backend_id| edge_id(&route_id, backend_id));
        let backend_to_endpoint_set_edge_id = backend_id
            .as_ref()
            .zip(endpoint_set_id.as_ref())
            .map(|(backend_id, endpoint_set_id)| edge_id(backend_id, endpoint_set_id));

        Self {
            route_kind,
            shard_key: topology_shard_key(&listener_id, &route_id, backend_name),
            dataplane_to_listener_edge_id: edge_id("plane:dataplane", &listener_id),
            listener_to_route_edge_id: edge_id(&listener_id, &route_id),
            listener_node_id: listener_id,
            route_node_id: route_id,
            backend_node_id: backend_id,
            endpoint_set_node_id: endpoint_set_id,
            route_to_backend_edge_id,
            backend_to_endpoint_set_edge_id,
        }
    }

    pub fn unmatched(listener_name: &str) -> Self {
        Self::from_parts(
            listener_name,
            UNMATCHED_TRAFFIC_ROUTE_KIND,
            UNMATCHED_TRAFFIC_ROUTE_NAMESPACE,
            UNMATCHED_TRAFFIC_ROUTE_NAME,
            "",
        )
    }

    pub fn as_ref(&self) -> TrafficTopologyRef<'_> {
        TrafficTopologyRef {
            route_kind: self.route_kind.as_ref(),
            shard_key: self.shard_key,
            listener_node_id: self.listener_node_id.as_str(),
            route_node_id: self.route_node_id.as_str(),
            backend_node_id: self.backend_node_id.as_deref(),
            endpoint_set_node_id: self.endpoint_set_node_id.as_deref(),
            dataplane_to_listener_edge_id: self.dataplane_to_listener_edge_id.as_str(),
            listener_to_route_edge_id: self.listener_to_route_edge_id.as_str(),
            route_to_backend_edge_id: self.route_to_backend_edge_id.as_deref(),
            backend_to_endpoint_set_edge_id: self.backend_to_endpoint_set_edge_id.as_deref(),
        }
    }
}

impl<'a> From<&'a TrafficTopology> for TrafficTopologyRef<'a> {
    fn from(topology: &'a TrafficTopology) -> Self {
        topology.as_ref()
    }
}

impl<'a> From<&'a TrafficObservation> for TrafficObservationRef<'a> {
    fn from(observation: &'a TrafficObservation) -> Self {
        Self {
            listener_name: observation.listener_name.as_str(),
            protocol: observation.protocol.as_str(),
            route_namespace: observation.route_namespace.as_str(),
            route_name: observation.route_name.as_str(),
            route_kind: observation.route_kind.as_str(),
            backend_name: observation.backend_name.as_str(),
            status: observation.status,
            latency_ms: observation.latency_ms,
            bytes_received: observation.bytes_received,
            bytes_sent: observation.bytes_sent,
            retry_attempts: observation.retry_attempts,
            retried_success: observation.retried_success,
            upstream_pool_hits: observation.upstream_pool_hits,
            upstream_pool_misses: observation.upstream_pool_misses,
            upstream_peer_build_failures: observation.upstream_peer_build_failures,
            upstream_connect_latency_ms: observation.upstream_connect_latency_ms,
            upstream_connect_latency_ms_max: observation.upstream_connect_latency_ms_max,
            upstream_connect_latency_ms_buckets: &observation.upstream_connect_latency_ms_buckets,
            response_flags: observation.response_flags.as_str(),
            runtime_ids: observation.runtime_ids,
        }
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct TrafficRuntimeIds {
    pub listener: Option<u64>,
    pub route: Option<u64>,
    pub backend: Option<u64>,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct TrafficSnapshot {
    pub generated_at_unix_ms: u128,
    pub total_events: u64,
    pub total_request_events: u64,
    pub total_bytes_received: u64,
    pub total_bytes_sent: u64,
    pub total_latency_ms: u64,
    pub max_latency_ms: u64,
    pub request_latency_ms_histograms: Vec<TrafficLabeledHistogram>,
    pub total_retried_events: u64,
    pub total_retry_attempts: u64,
    pub total_retried_success_events: u64,
    pub total_upstream_pool_hits: u64,
    pub total_upstream_pool_misses: u64,
    pub total_upstream_peer_build_failures: u64,
    pub total_upstream_connect_latency_observations: u64,
    pub total_upstream_connect_latency_ms: u64,
    pub max_upstream_connect_latency_ms: u64,
    pub upstream_connect_latency_ms_buckets: Vec<TrafficHistogramBucket>,
    pub total_upstream_tls_handshake_failures: u64,
    pub total_upstream_tls_handshake_failure_latency_observations: u64,
    pub total_upstream_tls_handshake_failure_latency_ms: u64,
    pub max_upstream_tls_handshake_failure_latency_ms: u64,
    pub upstream_tls_handshake_failure_latency_ms_buckets: Vec<TrafficHistogramBucket>,
    pub status_1xx: u64,
    pub status_2xx: u64,
    pub status_3xx: u64,
    pub status_4xx: u64,
    pub status_5xx: u64,
    pub status_other: u64,
    pub response_flags: BTreeMap<String, u64>,
    pub nodes: Vec<TrafficNodeStat>,
    pub edges: Vec<TrafficEdgeStat>,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct TrafficHistogramBucket {
    pub le: String,
    pub cumulative_count: u64,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct TrafficLabeledHistogram {
    pub listener: String,
    pub protocol: String,
    pub route_kind: String,
    pub status_class: String,
    pub response_flag: String,
    pub buckets: Vec<TrafficHistogramBucket>,
    pub sum: u64,
    pub count: u64,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct TrafficNodeStat {
    pub node_id: String,
    #[serde(
        rename = "runtimeId",
        skip_serializing_if = "Option::is_none",
        serialize_with = "serialize_runtime_id"
    )]
    pub runtime_id: Option<u64>,
    pub events: u64,
    pub bytes_received: u64,
    pub bytes_sent: u64,
    pub last_seen_unix_ms: u128,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct TrafficEdgeStat {
    pub edge_id: String,
    pub source: String,
    pub target: String,
    pub events: u64,
    pub bytes_received: u64,
    pub bytes_sent: u64,
    pub last_seen_unix_ms: u128,
}

#[derive(Debug, Clone)]
pub struct SharedTrafficStats {
    inner: Arc<TrafficStatsInner>,
}
