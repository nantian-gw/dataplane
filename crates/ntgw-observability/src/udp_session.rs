use std::{
    collections::BTreeMap,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
};

use parking_lot::RwLock;

pub type SharedUdpSessionStats = Arc<UdpSessionStats>;

type NamedCounterMap = RwLock<BTreeMap<String, u64>>;

#[derive(Debug, Clone, Default, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UdpSessionSnapshot {
    pub active_sessions_current: u64,
    pub active_sessions_by_listener: BTreeMap<String, u64>,
    pub queue_depth_current: u64,
    pub queue_depth_by_listener: BTreeMap<String, u64>,
    pub queue_overflow_dropped_total: u64,
    pub queue_overflow_dropped_by_listener: BTreeMap<String, u64>,
    pub idle_evictions_total: u64,
    pub idle_evictions_by_listener: BTreeMap<String, u64>,
}

#[derive(Debug, Default)]
pub struct UdpSessionStats {
    active_sessions_current: AtomicU64,
    active_sessions_by_listener: NamedCounterMap,
    queue_depth_current: AtomicU64,
    queue_depth_by_listener: NamedCounterMap,
    queue_overflow_dropped_total: AtomicU64,
    queue_overflow_dropped_by_listener: NamedCounterMap,
    idle_evictions_total: AtomicU64,
    idle_evictions_by_listener: NamedCounterMap,
}

impl UdpSessionStats {
    pub fn shared() -> SharedUdpSessionStats {
        Arc::new(Self::default())
    }

    pub fn snapshot(&self) -> UdpSessionSnapshot {
        UdpSessionSnapshot {
            active_sessions_current: self.active_sessions_current.load(Ordering::Relaxed),
            active_sessions_by_listener: snapshot_named(&self.active_sessions_by_listener),
            queue_depth_current: self.queue_depth_current.load(Ordering::Relaxed),
            queue_depth_by_listener: snapshot_named(&self.queue_depth_by_listener),
            queue_overflow_dropped_total: self.queue_overflow_dropped_total.load(Ordering::Relaxed),
            queue_overflow_dropped_by_listener: snapshot_named(
                &self.queue_overflow_dropped_by_listener,
            ),
            idle_evictions_total: self.idle_evictions_total.load(Ordering::Relaxed),
            idle_evictions_by_listener: snapshot_named(&self.idle_evictions_by_listener),
        }
    }

    pub fn observe_session_started(&self, listener: &str) {
        self.active_sessions_current.fetch_add(1, Ordering::Relaxed);
        increment_named(&self.active_sessions_by_listener, listener);
    }

    pub fn observe_session_ended(&self, listener: &str) {
        saturating_decrement(&self.active_sessions_current);
        decrement_named(&self.active_sessions_by_listener, listener);
    }

    pub fn observe_queue_enqueued(&self, listener: &str) {
        self.queue_depth_current.fetch_add(1, Ordering::Relaxed);
        increment_named(&self.queue_depth_by_listener, listener);
    }

    pub fn observe_queue_dequeued(&self, listener: &str) {
        saturating_decrement(&self.queue_depth_current);
        decrement_named(&self.queue_depth_by_listener, listener);
    }

    pub fn observe_queue_overflow_drop(&self, listener: &str) {
        self.queue_overflow_dropped_total
            .fetch_add(1, Ordering::Relaxed);
        increment_named(&self.queue_overflow_dropped_by_listener, listener);
    }

    pub fn observe_idle_eviction(&self, listener: &str) {
        self.idle_evictions_total.fetch_add(1, Ordering::Relaxed);
        increment_named(&self.idle_evictions_by_listener, listener);
    }
}

fn snapshot_named(items: &NamedCounterMap) -> BTreeMap<String, u64> {
    items.read().clone()
}

fn saturating_decrement(counter: &AtomicU64) {
    let _ = counter.fetch_update(Ordering::Relaxed, Ordering::Relaxed, |current| {
        Some(current.saturating_sub(1))
    });
}

fn increment_named(items: &NamedCounterMap, key: &str) {
    *items.write().entry(key.to_string()).or_default() += 1;
}

fn decrement_named(items: &NamedCounterMap, key: &str) {
    let mut items = items.write();
    let Some(value) = items.get_mut(key) else {
        return;
    };
    *value = value.saturating_sub(1);
    if *value == 0 {
        items.remove(key);
    }
}
