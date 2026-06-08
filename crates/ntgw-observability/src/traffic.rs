use std::{
    borrow::Cow,
    collections::{BTreeMap, HashMap},
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};

use parking_lot::RwLock;
use serde::{Serialize, Serializer};

use self::state::ObservedEdgeRef;
use self::topology::{
    backend_node_id, canonical_route_kind_ref, edge_id, endpoint_set_node_id, listener_node_id,
    parse_backend_name_ref, route_node_id, topology_shard_key,
};
use crate::epoch_millis;

mod state;
mod topology;

const MIN_TRAFFIC_SHARDS: usize = 4;
const MAX_TRAFFIC_SHARDS: usize = 64;
const DEFAULT_TRAFFIC_SHARDS: usize = 16;
const DEFAULT_TRAFFIC_NODE_LIMIT_PER_SHARD: usize = 1024;
const DEFAULT_TRAFFIC_EDGE_LIMIT_PER_SHARD: usize = 2048;
const UNKNOWN_TRAFFIC_LISTENER_LABEL: &str = "unknown";
const UNMATCHED_TRAFFIC_ROUTE_NAMESPACE: &str = "unmatched";
const UNMATCHED_TRAFFIC_ROUTE_NAME: &str = "no-route";
const UNMATCHED_TRAFFIC_ROUTE_KIND: &str = "unmatched";
pub const UPSTREAM_CONNECT_LATENCY_MS_BUCKET_BOUNDS: [u64; 11] =
    [1, 5, 10, 25, 50, 100, 250, 500, 1_000, 2_500, 5_000];
pub const UPSTREAM_CONNECT_LATENCY_MS_BUCKET_COUNT: usize =
    UPSTREAM_CONNECT_LATENCY_MS_BUCKET_BOUNDS.len() + 1;
pub const TRAFFIC_LATENCY_MS_BUCKET_BOUNDS: [u64; 16] = [
    1, 5, 10, 25, 50, 100, 250, 500, 1_000, 2_500, 5_000, 10_000, 30_000, 60_000, 120_000, 300_000,
];
pub const TRAFFIC_LATENCY_MS_BUCKET_COUNT: usize = TRAFFIC_LATENCY_MS_BUCKET_BOUNDS.len() + 1;
const NORMAL_RESPONSE_FLAG: &str = "none";

static TRAFFIC_THREAD_SHARD_COUNTER: AtomicUsize = AtomicUsize::new(0);

thread_local! {
    static TRAFFIC_THREAD_SHARD_KEY: usize =
        TRAFFIC_THREAD_SHARD_COUNTER.fetch_add(1, Ordering::Relaxed);
}

// ── Types: struct definitions, bucket functions ──
include!("traffic/types.incl.rs");

// ── Core stats: SharedTrafficStats, TrafficState, private helpers ──
include!("traffic/stats.incl.rs");

#[cfg(test)]
mod tests;
