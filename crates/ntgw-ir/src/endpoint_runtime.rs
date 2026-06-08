use std::{
    collections::{HashMap, hash_map::DefaultHasher},
    hash::{Hash, Hasher},
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::Instant,
};

use parking_lot::RwLock;

use crate::{BackendCluster, BackendEndpoint, EndpointRuntimeKey, EndpointRuntimeState};

const ENDPOINT_RUNTIME_SHARD_COUNT: usize = 32;
const ENDPOINT_RECOVERY_LATENCY_MS_BUCKET_BOUNDS: [u64; 10] = [
    50, 100, 250, 500, 1_000, 2_500, 5_000, 10_000, 30_000, 60_000,
];
const ENDPOINT_RECOVERY_LATENCY_MS_BUCKET_COUNT: usize =
    ENDPOINT_RECOVERY_LATENCY_MS_BUCKET_BOUNDS.len() + 1;

#[derive(Debug, Clone, Default, Eq, PartialEq)]
pub struct EndpointRuntimeHistogramBucket {
    pub le: String,
    pub cumulative_count: u64,
}

#[derive(Debug, Clone, Default, Eq, PartialEq)]
pub struct EndpointRuntimeSnapshot {
    pub tracked_endpoints: usize,
    pub passive_ejected_current: usize,
    pub active_unhealthy_current: usize,
    pub recovery_latency_ms_buckets: Vec<EndpointRuntimeHistogramBucket>,
    pub recovery_latency_ms_sum: u64,
    pub recovery_latency_ms_count: u64,
}

#[derive(Debug, Clone, Default, Eq, PartialEq)]
struct EndpointRecoveryLatencyStats {
    buckets: [u64; ENDPOINT_RECOVERY_LATENCY_MS_BUCKET_COUNT],
    sum: u64,
    count: u64,
}

#[derive(Debug)]
pub struct EndpointRuntimeStore {
    inner: Arc<EndpointRuntimeStoreInner>,
}

#[derive(Debug)]
struct EndpointRuntimeStoreInner {
    shards: Box<[RwLock<HashMap<EndpointRuntimeKey, EndpointRuntimeState>>]>,
    recovery_latency: RwLock<EndpointRecoveryLatencyStats>,
    tracked_states: AtomicUsize,
}

#[derive(Debug, Clone)]
pub struct EndpointRuntimeHandle {
    inner: Arc<EndpointRuntimeStoreInner>,
    key: EndpointRuntimeKey,
}

impl EndpointRuntimeHandle {
    pub fn record_failure(&self, now: Instant) {
        self.inner.update(self.key.clone(), |state| {
            state.record_failure(now);
            None
        });
    }

    pub fn record_success(&self) {
        self.inner.record_success(&self.key);
    }
}

impl Default for EndpointRuntimeStore {
    fn default() -> Self {
        let shards = (0..ENDPOINT_RUNTIME_SHARD_COUNT)
            .map(|_| RwLock::new(HashMap::new()))
            .collect::<Vec<_>>()
            .into_boxed_slice();

        Self {
            inner: Arc::new(EndpointRuntimeStoreInner {
                shards,
                recovery_latency: RwLock::new(EndpointRecoveryLatencyStats::default()),
                tracked_states: AtomicUsize::new(0),
            }),
        }
    }
}

impl Clone for EndpointRuntimeStore {
    fn clone(&self) -> Self {
        Self::from_map_and_recovery_stats(self.snapshot_map(), self.recovery_latency_stats())
    }
}

impl EndpointRuntimeStore {
    pub(crate) fn handle_for_backend(
        &self,
        backend_name: &str,
        endpoint: &BackendEndpoint,
    ) -> EndpointRuntimeHandle {
        self.handle_for_key(EndpointRuntimeKey::new(backend_name, endpoint))
    }

    pub(crate) fn record_failure_at(
        &self,
        backend_name: &str,
        endpoint: &BackendEndpoint,
        now: Instant,
    ) {
        self.inner
            .update(EndpointRuntimeKey::new(backend_name, endpoint), |state| {
                state.record_failure(now);
                None
            });
    }

    pub(crate) fn record_success(&self, backend_name: &str, endpoint: &BackendEndpoint) {
        self.inner
            .record_success(&EndpointRuntimeKey::new(backend_name, endpoint));
    }

    pub(crate) fn record_active_probe_failure(
        &self,
        backend_name: &str,
        endpoint: &BackendEndpoint,
        unhealthy_threshold: u32,
    ) {
        self.inner
            .update(EndpointRuntimeKey::new(backend_name, endpoint), |state| {
                state.record_active_probe_failure(unhealthy_threshold);
                None
            });
    }

    pub(crate) fn record_active_probe_success(
        &self,
        backend_name: &str,
        endpoint: &BackendEndpoint,
    ) {
        self.inner
            .update(EndpointRuntimeKey::new(backend_name, endpoint), |state| {
                state.record_active_probe_success()
            });
    }

    pub(crate) fn inherit_for_backends(&self, backends: &[BackendCluster]) -> Self {
        let mut next = HashMap::new();

        for cluster in backends {
            let backend_name = format!("{}/{}", cluster.namespace, cluster.name);
            for endpoint in &cluster.endpoints {
                let key = EndpointRuntimeKey::new(backend_name.as_str(), endpoint);
                let Some(state) = self.get_cloned(&key) else {
                    continue;
                };
                if state.is_default_state() {
                    continue;
                }
                next.insert(key, state);
            }
        }

        Self::from_map_and_recovery_stats(next, self.recovery_latency_stats())
    }

    pub(crate) fn get_cloned(&self, key: &EndpointRuntimeKey) -> Option<EndpointRuntimeState> {
        self.inner.shards[shard_index(key)].read().get(key).cloned()
    }

    pub(crate) fn has_tracked_states(&self) -> bool {
        self.inner.tracked_states.load(Ordering::Relaxed) != 0
    }

    pub(crate) fn snapshot_map(&self) -> HashMap<EndpointRuntimeKey, EndpointRuntimeState> {
        let mut out = HashMap::new();
        for shard in self.inner.shards.iter() {
            out.extend(
                shard
                    .read()
                    .iter()
                    .map(|(key, state)| (key.clone(), state.clone())),
            );
        }
        out
    }

    pub(crate) fn snapshot(&self, now: Instant) -> EndpointRuntimeSnapshot {
        let mut snapshot = EndpointRuntimeSnapshot::default();
        for shard in self.inner.shards.iter() {
            let states = shard.read();
            snapshot.tracked_endpoints += states.len();
            snapshot.passive_ejected_current += states
                .values()
                .filter(|state| state.is_ejected_at(now))
                .count();
            snapshot.active_unhealthy_current += states
                .values()
                .filter(|state| state.is_active_unhealthy())
                .count();
        }
        let recovery_latency = self.recovery_latency_stats();
        snapshot.recovery_latency_ms_buckets = recovery_latency.bucket_snapshot();
        snapshot.recovery_latency_ms_sum = recovery_latency.sum;
        snapshot.recovery_latency_ms_count = recovery_latency.count;
        snapshot
    }

    fn recovery_latency_stats(&self) -> EndpointRecoveryLatencyStats {
        self.inner.recovery_latency.read().clone()
    }

    fn from_map_and_recovery_stats(
        states: HashMap<EndpointRuntimeKey, EndpointRuntimeState>,
        recovery_latency: EndpointRecoveryLatencyStats,
    ) -> Self {
        let store = Self::default();
        *store.inner.recovery_latency.write() = recovery_latency;
        for (key, state) in states {
            store.insert_state(key, state);
        }
        store
    }

    fn handle_for_key(&self, key: EndpointRuntimeKey) -> EndpointRuntimeHandle {
        EndpointRuntimeHandle {
            inner: Arc::clone(&self.inner),
            key,
        }
    }

    fn insert_state(&self, key: EndpointRuntimeKey, state: EndpointRuntimeState) {
        self.inner.shards[shard_index(&key)]
            .write()
            .insert(key, state);
        self.inner.tracked_states.fetch_add(1, Ordering::Relaxed);
    }
}

impl EndpointRuntimeStoreInner {
    fn record_success(&self, key: &EndpointRuntimeKey) {
        if self.tracked_states.load(Ordering::Relaxed) == 0 {
            return;
        }

        let recovered_latency_ms = {
            let shard = &self.shards[shard_index(key)];
            let mut states = shard.write();
            let Some(state) = states.get_mut(key) else {
                return;
            };
            let recovered_latency_ms = state.record_success();
            if state.is_default_state() {
                states.remove(key);
                self.tracked_states.fetch_sub(1, Ordering::Relaxed);
            }
            recovered_latency_ms
        };

        if let Some(latency_ms) = recovered_latency_ms {
            self.recovery_latency.write().observe(latency_ms);
        }
    }

    fn update<F>(&self, key: EndpointRuntimeKey, update: F)
    where
        F: FnOnce(&mut EndpointRuntimeState) -> Option<u64>,
    {
        let recovered_latency_ms = {
            let shard = &self.shards[shard_index(&key)];
            let mut states = shard.write();
            let existed = states.contains_key(&key);
            let (remove, recovered_latency_ms) = {
                let state = states.entry(key.clone()).or_default();
                let recovered_latency_ms = update(state);
                (state.is_default_state(), recovered_latency_ms)
            };
            if remove {
                states.remove(&key);
                if existed {
                    self.tracked_states.fetch_sub(1, Ordering::Relaxed);
                }
            } else if !existed {
                self.tracked_states.fetch_add(1, Ordering::Relaxed);
            }
            recovered_latency_ms
        };
        if let Some(latency_ms) = recovered_latency_ms {
            self.recovery_latency.write().observe(latency_ms);
        }
    }
}

impl EndpointRecoveryLatencyStats {
    fn observe(&mut self, latency_ms: u64) {
        let index = endpoint_recovery_latency_ms_bucket_index(latency_ms);
        self.buckets[index] = self.buckets[index].saturating_add(1);
        self.sum = self.sum.saturating_add(latency_ms);
        self.count = self.count.saturating_add(1);
    }

    fn bucket_snapshot(&self) -> Vec<EndpointRuntimeHistogramBucket> {
        let mut cumulative_count = 0u64;
        let mut buckets = Vec::with_capacity(ENDPOINT_RECOVERY_LATENCY_MS_BUCKET_COUNT);
        for (index, bound) in ENDPOINT_RECOVERY_LATENCY_MS_BUCKET_BOUNDS
            .iter()
            .enumerate()
        {
            cumulative_count = cumulative_count.saturating_add(self.buckets[index]);
            buckets.push(EndpointRuntimeHistogramBucket {
                le: bound.to_string(),
                cumulative_count,
            });
        }
        cumulative_count = cumulative_count
            .saturating_add(self.buckets[ENDPOINT_RECOVERY_LATENCY_MS_BUCKET_COUNT - 1]);
        buckets.push(EndpointRuntimeHistogramBucket {
            le: "+Inf".to_string(),
            cumulative_count,
        });
        buckets
    }
}

fn endpoint_recovery_latency_ms_bucket_index(latency_ms: u64) -> usize {
    ENDPOINT_RECOVERY_LATENCY_MS_BUCKET_BOUNDS
        .iter()
        .position(|bound| latency_ms <= *bound)
        .unwrap_or(ENDPOINT_RECOVERY_LATENCY_MS_BUCKET_BOUNDS.len())
}

fn shard_index(key: &EndpointRuntimeKey) -> usize {
    let mut hasher = DefaultHasher::new();
    key.hash(&mut hasher);
    (hasher.finish() as usize) % ENDPOINT_RUNTIME_SHARD_COUNT
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reports_whether_any_endpoint_runtime_state_is_tracked() {
        let store = EndpointRuntimeStore::default();
        let endpoint = BackendEndpoint {
            address: "10.0.0.10".to_string(),
            port: 8080,
            healthy: true,
        };

        assert!(!store.has_tracked_states());

        store.record_failure_at("default/orders:8080", &endpoint, Instant::now());
        assert!(store.has_tracked_states());

        store.record_success("default/orders:8080", &endpoint);
        assert!(!store.has_tracked_states());
    }
}
