use super::state::ObservedEdge;
use super::{
    DEFAULT_TRAFFIC_EDGE_LIMIT_PER_SHARD, DEFAULT_TRAFFIC_NODE_LIMIT_PER_SHARD, SharedTrafficStats,
    TrafficLatencyLabelRef, TrafficObservation, TrafficObservationRef, TrafficRuntimeIds,
    TrafficState, TrafficTopology, UPSTREAM_CONNECT_LATENCY_MS_BUCKET_COUNT,
    observe_request_latency_ref, observe_response_flags, upstream_connect_latency_ms_bucket_index,
};

include!("tests/topology.rs");
include!("tests/shard_merge.rs");
include!("tests/capacity.rs");
include!("tests/eviction.rs");
