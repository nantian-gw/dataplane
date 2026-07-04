const FLUSH_THRESHOLD: usize = 1024;
const _FLUSH_INTERVAL_MS: u64 = 100;

#[derive(Debug)]
struct TrafficStatsInner {
    shard_mask: usize,
    node_limit_per_shard: usize,
    edge_limit_per_shard: usize,
    shards: Vec<RwLock<TrafficState>>,
}

#[derive(Debug, Default)]
struct TrafficState {
    total_events: u64,
    total_request_events: u64,
    total_bytes_received: u64,
    total_bytes_sent: u64,
    total_latency_ms: u64,
    max_latency_ms: u64,
    request_latency_ms_histograms: Vec<(TrafficLatencyLabels, TrafficHistogramState)>,
    total_retried_events: u64,
    total_retry_attempts: u64,
    total_retried_success_events: u64,
    total_upstream_pool_hits: u64,
    total_upstream_pool_misses: u64,
    total_upstream_peer_build_failures: u64,
    total_upstream_connect_latency_observations: u64,
    total_upstream_connect_latency_ms: u64,
    max_upstream_connect_latency_ms: u64,
    upstream_connect_latency_ms_buckets: [u64; UPSTREAM_CONNECT_LATENCY_MS_BUCKET_COUNT],
    total_upstream_tls_handshake_failures: u64,
    total_upstream_tls_handshake_failure_latency_observations: u64,
    total_upstream_tls_handshake_failure_latency_ms: u64,
    max_upstream_tls_handshake_failure_latency_ms: u64,
    upstream_tls_handshake_failure_latency_ms_buckets:
        [u64; UPSTREAM_CONNECT_LATENCY_MS_BUCKET_COUNT],
    status_1xx: u64,
    status_2xx: u64,
    status_3xx: u64,
    status_4xx: u64,
    status_5xx: u64,
    status_other: u64,
    response_flags: BTreeMap<String, u64>,
    nodes: HashMap<String, TrafficNodeStat>,
    edges: HashMap<String, TrafficEdgeStat>,
}

#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd)]
struct TrafficLatencyLabels {
    listener: String,
    protocol: String,
    route_kind: String,
    status_class: String,
    response_flag: String,
}

#[derive(Debug, Clone, Copy)]
struct TrafficLatencyLabelRef<'a> {
    listener: &'a str,
    protocol: &'a str,
    route_kind: &'a str,
    status_class: &'a str,
    response_flag: &'a str,
}

#[derive(Debug, Clone)]
struct TrafficHistogramState {
    buckets: [u64; TRAFFIC_LATENCY_MS_BUCKET_COUNT],
    sum: u64,
    count: u64,
}

impl Default for TrafficHistogramState {
    fn default() -> Self {
        Self {
            buckets: [0; TRAFFIC_LATENCY_MS_BUCKET_COUNT],
            sum: 0,
            count: 0,
        }
    }
}

impl SharedTrafficStats {
    pub fn shared() -> Self {
        Self::default()
    }

    pub(crate) fn with_shard_count(shard_count: usize) -> Self {
        Self {
            inner: Arc::new(TrafficStatsInner::new(shard_count)),
            shared_buffer: Arc::new(parking_lot::Mutex::new(Vec::with_capacity(FLUSH_THRESHOLD))),
        }
    }

    pub fn observe(&self, observation: TrafficObservation) {
        self.observe_ref(TrafficObservationRef::from(&observation));
    }

    pub fn observe_ref(&self, observation: TrafficObservationRef<'_>) {
        self.observe_ref_with_topology(observation, None);
    }

    pub fn observe_ref_with_topology(
        &self,
        observation: TrafficObservationRef<'_>,
        topology: Option<TrafficTopologyRef<'_>>,
    ) {
        if topology.is_some() {
            return self.observe_ref_direct(observation, topology);
        }
        let mut buf = match self.shared_buffer.try_lock() {
            Some(buf) => buf,
            None => return,
        };
        buf.push(observation.to_owned());
        let should_flush = buf.len() >= FLUSH_THRESHOLD;
        drop(buf);
        if should_flush {
            self.flush_batch();
        }
    }

    fn observe_ref_direct(
        &self,
        observation: TrafficObservationRef<'_>,
        topology: Option<TrafficTopologyRef<'_>>,
    ) {
        let now = epoch_millis();
        let listener_name =
            label_or_fallback(observation.listener_name, UNKNOWN_TRAFFIC_LISTENER_LABEL);
        let route_namespace = label_or_fallback(
            observation.route_namespace,
            UNMATCHED_TRAFFIC_ROUTE_NAMESPACE,
        );
        let route_name = label_or_fallback(observation.route_name, UNMATCHED_TRAFFIC_ROUTE_NAME);
        let route_kind = label_or_fallback(observation.route_kind, UNMATCHED_TRAFFIC_ROUTE_KIND);
        let owned_topology;
        let topology = if let Some(topology) = topology {
            topology
        } else {
            owned_topology = TrafficTopology::from_parts(
                listener_name.as_ref(),
                route_kind.as_ref(),
                route_namespace.as_ref(),
                route_name.as_ref(),
                observation.backend_name,
            );
            owned_topology.as_ref()
        };
        let worker_shard_key = traffic_thread_shard_key();

        let mut state = self
            .inner
            .shard_for(topology.shard_key, worker_shard_key)
            .write();
        let node_limit_per_shard = self.inner.node_limit_per_shard;
        let edge_limit_per_shard = self.inner.edge_limit_per_shard;
        state.total_events += 1;
        state.total_bytes_received = state
            .total_bytes_received
            .saturating_add(observation.bytes_received);
        state.total_bytes_sent = state
            .total_bytes_sent
            .saturating_add(observation.bytes_sent);
        state.total_latency_ms = state
            .total_latency_ms
            .saturating_add(observation.latency_ms);
        state.max_latency_ms = state.max_latency_ms.max(observation.latency_ms);
        let protocol = canonical_protocol_label_cow(observation.protocol);
        if is_request_protocol(protocol.as_ref()) {
            let response_flag = response_flag_label_cow(observation.response_flags);
            state.total_request_events = state.total_request_events.saturating_add(1);
            observe_request_latency_ref(
                &mut state,
                TrafficLatencyLabelRef {
                    listener: listener_name.as_ref(),
                    protocol: protocol.as_ref(),
                    route_kind: topology.route_kind,
                    status_class: status_class_label(observation.status),
                    response_flag: response_flag.as_ref(),
                },
                observation.latency_ms,
            );
            match observation.status {
                Some(100..=199) => state.status_1xx += 1,
                Some(200..=299) => state.status_2xx += 1,
                Some(300..=399) => state.status_3xx += 1,
                Some(400..=499) => state.status_4xx += 1,
                Some(500..=599) => state.status_5xx += 1,
                _ => state.status_other += 1,
            }
            observe_response_flags(&mut state, observation.response_flags);
            if observation.retry_attempts > 0 {
                state.total_retried_events += 1;
                state.total_retry_attempts = state
                    .total_retry_attempts
                    .saturating_add(observation.retry_attempts as u64);
                if observation.retried_success {
                    state.total_retried_success_events += 1;
                }
            }

            state.total_upstream_pool_hits = state
                .total_upstream_pool_hits
                .saturating_add(observation.upstream_pool_hits as u64);
            state.total_upstream_pool_misses = state
                .total_upstream_pool_misses
                .saturating_add(observation.upstream_pool_misses as u64);
            state.total_upstream_peer_build_failures = state
                .total_upstream_peer_build_failures
                .saturating_add(observation.upstream_peer_build_failures as u64);
            if has_upstream_connect_latency_observation(&observation) {
                for (index, count) in observation
                    .upstream_connect_latency_ms_buckets
                    .iter()
                    .enumerate()
                {
                    state.upstream_connect_latency_ms_buckets[index] = state
                        .upstream_connect_latency_ms_buckets[index]
                        .saturating_add(*count as u64);
                    state.total_upstream_connect_latency_observations = state
                        .total_upstream_connect_latency_observations
                        .saturating_add(*count as u64);
                }
                state.total_upstream_connect_latency_ms = state
                    .total_upstream_connect_latency_ms
                    .saturating_add(observation.upstream_connect_latency_ms);
                state.max_upstream_connect_latency_ms = state
                    .max_upstream_connect_latency_ms
                    .max(observation.upstream_connect_latency_ms_max);
            }
        }
        state.observe_node_ref(
            "plane:dataplane",
            None,
            observation.bytes_received,
            observation.bytes_sent,
            now,
            node_limit_per_shard,
        );
        state.observe_node_ref(
            topology.listener_node_id,
            observation.runtime_ids.listener,
            observation.bytes_received,
            observation.bytes_sent,
            now,
            node_limit_per_shard,
        );
        state.observe_node_ref(
            topology.route_node_id,
            observation.runtime_ids.route,
            observation.bytes_received,
            observation.bytes_sent,
            now,
            node_limit_per_shard,
        );

        state.observe_edge_ref(
            ObservedEdgeRef {
                edge_id: topology.dataplane_to_listener_edge_id,
                source: "plane:dataplane",
                target: topology.listener_node_id,
                bytes_received: observation.bytes_received,
                bytes_sent: observation.bytes_sent,
            },
            now,
            edge_limit_per_shard,
        );
        state.observe_edge_ref(
            ObservedEdgeRef {
                edge_id: topology.listener_to_route_edge_id,
                source: topology.listener_node_id,
                target: topology.route_node_id,
                bytes_received: observation.bytes_received,
                bytes_sent: observation.bytes_sent,
            },
            now,
            edge_limit_per_shard,
        );

        if let Some(backend_id) = topology.backend_node_id {
            state.observe_node_ref(
                backend_id,
                observation.runtime_ids.backend,
                observation.bytes_received,
                observation.bytes_sent,
                now,
                node_limit_per_shard,
            );
            if let Some(route_to_backend_edge_id) = topology.route_to_backend_edge_id {
                state.observe_edge_ref(
                    ObservedEdgeRef {
                        edge_id: route_to_backend_edge_id,
                        source: topology.route_node_id,
                        target: backend_id,
                        bytes_received: observation.bytes_received,
                        bytes_sent: observation.bytes_sent,
                    },
                    now,
                    edge_limit_per_shard,
                );
            }

            if let Some(endpoint_set_id) = topology.endpoint_set_node_id {
                state.observe_node_ref(
                    endpoint_set_id,
                    None,
                    observation.bytes_received,
                    observation.bytes_sent,
                    now,
                    node_limit_per_shard,
                );
                if let Some(backend_to_endpoint_set_edge_id) =
                    topology.backend_to_endpoint_set_edge_id
                {
                    state.observe_edge_ref(
                        ObservedEdgeRef {
                            edge_id: backend_to_endpoint_set_edge_id,
                            source: backend_id,
                            target: endpoint_set_id,
                            bytes_received: observation.bytes_received,
                            bytes_sent: observation.bytes_sent,
                        },
                        now,
                        edge_limit_per_shard,
                    );
                }
            }
        }
    }

    pub fn observe_upstream_tls_handshake_failure(&self, latency_ms: Option<u64>) {
        let mut state = self.inner.shards[0].write();
        state.total_upstream_tls_handshake_failures = state
            .total_upstream_tls_handshake_failures
            .saturating_add(1);
        let Some(latency_ms) = latency_ms else {
            return;
        };

        state.total_upstream_tls_handshake_failure_latency_observations = state
            .total_upstream_tls_handshake_failure_latency_observations
            .saturating_add(1);
        state.total_upstream_tls_handshake_failure_latency_ms = state
            .total_upstream_tls_handshake_failure_latency_ms
            .saturating_add(latency_ms);
        state.max_upstream_tls_handshake_failure_latency_ms = state
            .max_upstream_tls_handshake_failure_latency_ms
            .max(latency_ms);
        let bucket_index = upstream_connect_latency_ms_bucket_index(latency_ms);
        state.upstream_tls_handshake_failure_latency_ms_buckets[bucket_index] =
            state.upstream_tls_handshake_failure_latency_ms_buckets[bucket_index].saturating_add(1);
    }

    fn flush_batch(&self) {
        let batch: Vec<TrafficObservation> = {
            let mut buf = self.shared_buffer.lock();
            std::mem::take(&mut *buf)
        };
        if batch.is_empty() {
            return;
        }
        for obs in &batch {
            let obs_ref = TrafficObservationRef::from(obs);
            self.observe_ref_direct(obs_ref, None);
        }
    }

    pub fn snapshot(&self) -> TrafficSnapshot {
        self.flush_batch();
        let mut state = TrafficState::default();
        for shard in &self.inner.shards {
            let shard = shard.read();
            state.merge_from(&shard);
        }

        let mut nodes = state.nodes.values().cloned().collect::<Vec<_>>();
        let mut edges = state.edges.values().cloned().collect::<Vec<_>>();
        nodes.sort_by(|left, right| left.node_id.cmp(&right.node_id));
        edges.sort_by(|left, right| left.edge_id.cmp(&right.edge_id));
        TrafficSnapshot {
            generated_at_unix_ms: epoch_millis(),
            total_events: state.total_events,
            total_request_events: state.total_request_events,
            total_bytes_received: state.total_bytes_received,
            total_bytes_sent: state.total_bytes_sent,
            total_latency_ms: state.total_latency_ms,
            max_latency_ms: state.max_latency_ms,
            request_latency_ms_histograms: request_latency_histogram_snapshot(
                &state.request_latency_ms_histograms,
            ),
            total_retried_events: state.total_retried_events,
            total_retry_attempts: state.total_retry_attempts,
            total_retried_success_events: state.total_retried_success_events,
            total_upstream_pool_hits: state.total_upstream_pool_hits,
            total_upstream_pool_misses: state.total_upstream_pool_misses,
            total_upstream_peer_build_failures: state.total_upstream_peer_build_failures,
            total_upstream_connect_latency_observations: state
                .total_upstream_connect_latency_observations,
            total_upstream_connect_latency_ms: state.total_upstream_connect_latency_ms,
            max_upstream_connect_latency_ms: state.max_upstream_connect_latency_ms,
            upstream_connect_latency_ms_buckets: upstream_connect_latency_bucket_snapshot(
                &state.upstream_connect_latency_ms_buckets,
            ),
            total_upstream_tls_handshake_failures: state.total_upstream_tls_handshake_failures,
            total_upstream_tls_handshake_failure_latency_observations: state
                .total_upstream_tls_handshake_failure_latency_observations,
            total_upstream_tls_handshake_failure_latency_ms: state
                .total_upstream_tls_handshake_failure_latency_ms,
            max_upstream_tls_handshake_failure_latency_ms: state
                .max_upstream_tls_handshake_failure_latency_ms,
            upstream_tls_handshake_failure_latency_ms_buckets:
                upstream_connect_latency_bucket_snapshot(
                    &state.upstream_tls_handshake_failure_latency_ms_buckets,
                ),
            status_1xx: state.status_1xx,
            status_2xx: state.status_2xx,
            status_3xx: state.status_3xx,
            status_4xx: state.status_4xx,
            status_5xx: state.status_5xx,
            status_other: state.status_other,
            response_flags: state.response_flags,
            nodes,
            edges,
        }
    }
}

impl Default for SharedTrafficStats {
    fn default() -> Self {
        Self {
            inner: Arc::new(TrafficStatsInner::new(default_traffic_shard_count())),
            shared_buffer: Arc::new(parking_lot::Mutex::new(Vec::with_capacity(FLUSH_THRESHOLD))),
        }
    }
}

impl TrafficStatsInner {
    fn new(shard_count: usize) -> Self {
        Self::with_limits(
            shard_count,
            DEFAULT_TRAFFIC_NODE_LIMIT_PER_SHARD,
            DEFAULT_TRAFFIC_EDGE_LIMIT_PER_SHARD,
        )
    }

    fn with_limits(
        shard_count: usize,
        node_limit_per_shard: usize,
        edge_limit_per_shard: usize,
    ) -> Self {
        let shard_count = shard_count.max(1).next_power_of_two();
        let mut shards = Vec::with_capacity(shard_count);
        for _ in 0..shard_count {
            shards.push(RwLock::new(TrafficState::default()));
        }

        Self {
            shard_mask: shard_count - 1,
            node_limit_per_shard,
            edge_limit_per_shard,
            shards,
        }
    }

    fn shard_for(&self, topology_shard_key: u64, worker_shard_key: usize) -> &RwLock<TrafficState> {
        let index = (topology_shard_key as usize).wrapping_add(worker_shard_key) & self.shard_mask;
        &self.shards[index]
    }
}

fn label_or_fallback<'a>(value: &'a str, fallback: &'static str) -> Cow<'a, str> {
    if value.trim().is_empty() {
        Cow::Borrowed(fallback)
    } else {
        Cow::Borrowed(value)
    }
}

fn traffic_thread_shard_key() -> usize {
    TRAFFIC_THREAD_SHARD_KEY.with(|key| *key)
}

fn has_upstream_connect_latency_observation(observation: &TrafficObservationRef<'_>) -> bool {
    observation.upstream_pool_misses > 0
        || observation.upstream_connect_latency_ms > 0
        || observation.upstream_connect_latency_ms_max > 0
        || observation.upstream_connect_latency_ms_buckets[0] > 0
}

fn default_traffic_shard_count() -> usize {
    std::thread::available_parallelism()
        .map(|parallelism| {
            let target = (parallelism.get() * 4).clamp(MIN_TRAFFIC_SHARDS, MAX_TRAFFIC_SHARDS);
            target.next_power_of_two()
        })
        .unwrap_or(DEFAULT_TRAFFIC_SHARDS)
}

fn response_flag_values(flags: &str) -> impl Iterator<Item = &str> {
    flags
        .split(',')
        .map(str::trim)
        .filter(|flag| !flag.is_empty())
}

impl TrafficLatencyLabels {
    fn from_ref(labels: TrafficLatencyLabelRef<'_>) -> Self {
        Self {
            listener: labels.listener.to_string(),
            protocol: labels.protocol.to_string(),
            route_kind: labels.route_kind.to_string(),
            status_class: labels.status_class.to_string(),
            response_flag: labels.response_flag.to_string(),
        }
    }

    fn matches_ref(&self, labels: TrafficLatencyLabelRef<'_>) -> bool {
        self.listener == labels.listener
            && self.protocol == labels.protocol
            && self.route_kind == labels.route_kind
            && self.status_class == labels.status_class
            && self.response_flag == labels.response_flag
    }
}

fn observe_request_latency_ref(
    state: &mut TrafficState,
    labels: TrafficLatencyLabelRef<'_>,
    latency_ms: u64,
) {
    if let Some((_, histogram)) = state
        .request_latency_ms_histograms
        .iter_mut()
        .find(|(existing, _)| existing.matches_ref(labels))
    {
        observe_request_latency_histogram(histogram, latency_ms);
        return;
    }

    let mut histogram = TrafficHistogramState::default();
    observe_request_latency_histogram(&mut histogram, latency_ms);
    state
        .request_latency_ms_histograms
        .push((TrafficLatencyLabels::from_ref(labels), histogram));
}

fn merge_request_latency_histogram(
    histograms: &mut Vec<(TrafficLatencyLabels, TrafficHistogramState)>,
    labels: &TrafficLatencyLabels,
    source: &TrafficHistogramState,
) {
    if let Some((_, histogram)) = histograms
        .iter_mut()
        .find(|(existing, _)| existing == labels)
    {
        histogram.sum = histogram.sum.saturating_add(source.sum);
        histogram.count = histogram.count.saturating_add(source.count);
        for (index, count) in source.buckets.iter().enumerate() {
            histogram.buckets[index] = histogram.buckets[index].saturating_add(*count);
        }
        return;
    }

    histograms.push((labels.clone(), source.clone()));
}

fn observe_request_latency_histogram(histogram: &mut TrafficHistogramState, latency_ms: u64) {
    histogram.count = histogram.count.saturating_add(1);
    histogram.sum = histogram.sum.saturating_add(latency_ms);
    let bucket_index = traffic_latency_ms_bucket_index(latency_ms);
    histogram.buckets[bucket_index] = histogram.buckets[bucket_index].saturating_add(1);
}

fn canonical_protocol_label_cow(protocol: &str) -> Cow<'_, str> {
    let trimmed = protocol.trim();
    let trimmed = trimmed
        .strip_prefix("LISTENER_PROTOCOL_")
        .unwrap_or(trimmed)
        .trim();
    if trimmed.is_empty() {
        Cow::Borrowed("unknown")
    } else if trimmed.bytes().all(|byte| !byte.is_ascii_lowercase()) {
        Cow::Borrowed(trimmed)
    } else {
        Cow::Owned(trimmed.to_ascii_uppercase())
    }
}

fn status_class_label(status: Option<u16>) -> &'static str {
    match status {
        Some(100..=199) => "1xx",
        Some(200..=299) => "2xx",
        Some(300..=399) => "3xx",
        Some(400..=499) => "4xx",
        Some(500..=599) => "5xx",
        _ => "other",
    }
}

fn is_request_protocol(protocol: &str) -> bool {
    matches!(
        protocol,
        "HTTP" | "HTTPS" | "GRPC" | "GRPCS" | "H2C" | "HTTP2" | "HTTP/2"
    )
}

fn observe_response_flags(state: &mut TrafficState, flags: &str) {
    let mut has_response_flag = false;
    for flag in response_flag_values(flags) {
        has_response_flag = true;
        increment_response_flag(&mut state.response_flags, flag);
    }
    if !has_response_flag {
        increment_response_flag(&mut state.response_flags, NORMAL_RESPONSE_FLAG);
    }
}

fn increment_response_flag(response_flags: &mut BTreeMap<String, u64>, flag: &str) {
    if let Some(events) = response_flags.get_mut(flag) {
        *events = events.saturating_add(1);
    } else {
        response_flags.insert(flag.to_string(), 1);
    }
}

fn response_flag_label_cow(flags: &str) -> Cow<'_, str> {
    let mut values = response_flag_values(flags);
    let Some(first) = values.next() else {
        return Cow::Borrowed(NORMAL_RESPONSE_FLAG);
    };
    if values.next().is_some() {
        Cow::Borrowed("multiple")
    } else {
        Cow::Borrowed(first)
    }
}

fn serialize_runtime_id<S>(runtime_id: &Option<u64>, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    match runtime_id {
        Some(runtime_id) => serializer.serialize_str(&format!("{runtime_id:016x}")),
        None => serializer.serialize_none(),
    }
}

fn request_latency_histogram_snapshot(
    histograms: &[(TrafficLatencyLabels, TrafficHistogramState)],
) -> Vec<TrafficLabeledHistogram> {
    let mut sorted = histograms.iter().collect::<Vec<_>>();
    sorted.sort_by(|(left, _), (right, _)| left.cmp(right));
    sorted
        .into_iter()
        .map(|(labels, histogram)| TrafficLabeledHistogram {
            listener: labels.listener.clone(),
            protocol: labels.protocol.clone(),
            route_kind: labels.route_kind.clone(),
            status_class: labels.status_class.clone(),
            response_flag: labels.response_flag.clone(),
            buckets: traffic_latency_bucket_snapshot(&histogram.buckets),
            sum: histogram.sum,
            count: histogram.count,
        })
        .collect()
}

fn upstream_connect_latency_bucket_snapshot(
    counts: &[u64; UPSTREAM_CONNECT_LATENCY_MS_BUCKET_COUNT],
) -> Vec<TrafficHistogramBucket> {
    let mut cumulative_count = 0u64;
    let mut buckets = Vec::with_capacity(UPSTREAM_CONNECT_LATENCY_MS_BUCKET_COUNT);
    for (index, bound) in UPSTREAM_CONNECT_LATENCY_MS_BUCKET_BOUNDS.iter().enumerate() {
        cumulative_count = cumulative_count.saturating_add(counts[index]);
        buckets.push(TrafficHistogramBucket {
            le: bound.to_string(),
            cumulative_count,
        });
    }
    cumulative_count =
        cumulative_count.saturating_add(counts[UPSTREAM_CONNECT_LATENCY_MS_BUCKET_COUNT - 1]);
    buckets.push(TrafficHistogramBucket {
        le: "+Inf".to_string(),
        cumulative_count,
    });
    buckets
}

fn traffic_latency_bucket_snapshot(
    counts: &[u64; TRAFFIC_LATENCY_MS_BUCKET_COUNT],
) -> Vec<TrafficHistogramBucket> {
    let mut cumulative_count = 0u64;
    let mut buckets = Vec::with_capacity(TRAFFIC_LATENCY_MS_BUCKET_COUNT);
    for (index, bound) in TRAFFIC_LATENCY_MS_BUCKET_BOUNDS.iter().enumerate() {
        cumulative_count = cumulative_count.saturating_add(counts[index]);
        buckets.push(TrafficHistogramBucket {
            le: bound.to_string(),
            cumulative_count,
        });
    }
    cumulative_count = cumulative_count.saturating_add(counts[TRAFFIC_LATENCY_MS_BUCKET_COUNT - 1]);
    buckets.push(TrafficHistogramBucket {
        le: "+Inf".to_string(),
        cumulative_count,
    });
    buckets
}
