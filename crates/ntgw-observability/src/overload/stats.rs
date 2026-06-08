use std::{
    collections::BTreeMap,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
};

use parking_lot::RwLock;

use super::budget::BudgetScope;

pub type SharedOverloadStats = Arc<OverloadStats>;

pub(super) type NamedCounterMap = RwLock<BTreeMap<String, u64>>;

#[derive(Debug, Clone, Default, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OverloadSnapshot {
    pub http_global_inflight_current: u64,
    pub http_listener_inflight_current: BTreeMap<String, u64>,
    pub http_route_inflight_current: BTreeMap<String, u64>,
    pub http_rejected_total: u64,
    pub http_rejected_global_total: u64,
    pub http_rejected_listener_total: u64,
    pub http_rejected_route_total: u64,
    pub http_rejected_listener_by_name: BTreeMap<String, u64>,
    pub http_rejected_route_by_name: BTreeMap<String, u64>,
    pub tcp_global_connections_current: u64,
    pub tcp_listener_connections_current: BTreeMap<String, u64>,
    pub tcp_rejected_total: u64,
    pub tcp_rejected_global_total: u64,
    pub tcp_rejected_listener_total: u64,
    pub tcp_rejected_listener_by_name: BTreeMap<String, u64>,
    pub udp_global_datagrams_current: u64,
    pub udp_listener_datagrams_current: BTreeMap<String, u64>,
    pub udp_rejected_total: u64,
    pub udp_rejected_global_total: u64,
    pub udp_rejected_listener_total: u64,
    pub udp_rejected_listener_by_name: BTreeMap<String, u64>,
}

#[derive(Debug, Default)]
pub struct OverloadStats {
    pub(super) http_global_inflight_current: AtomicU64,
    pub(super) http_listener_inflight_current: NamedCounterMap,
    pub(super) http_route_inflight_current: NamedCounterMap,
    pub(super) http_rejected_total: AtomicU64,
    pub(super) http_rejected_global_total: AtomicU64,
    pub(super) http_rejected_listener_total: AtomicU64,
    pub(super) http_rejected_route_total: AtomicU64,
    pub(super) http_rejected_listener_by_name: NamedCounterMap,
    pub(super) http_rejected_route_by_name: NamedCounterMap,
    pub(super) tcp_global_connections_current: AtomicU64,
    pub(super) tcp_listener_connections_current: NamedCounterMap,
    pub(super) tcp_rejected_total: AtomicU64,
    pub(super) tcp_rejected_global_total: AtomicU64,
    pub(super) tcp_rejected_listener_total: AtomicU64,
    pub(super) tcp_rejected_listener_by_name: NamedCounterMap,
    pub(super) udp_global_datagrams_current: AtomicU64,
    pub(super) udp_listener_datagrams_current: NamedCounterMap,
    pub(super) udp_rejected_total: AtomicU64,
    pub(super) udp_rejected_global_total: AtomicU64,
    pub(super) udp_rejected_listener_total: AtomicU64,
    pub(super) udp_rejected_listener_by_name: NamedCounterMap,
}

impl OverloadStats {
    pub fn shared() -> SharedOverloadStats {
        Arc::new(Self::default())
    }

    pub fn snapshot(&self) -> OverloadSnapshot {
        OverloadSnapshot {
            http_global_inflight_current: self.http_global_inflight_current.load(Ordering::Relaxed),
            http_listener_inflight_current: snapshot_named(&self.http_listener_inflight_current),
            http_route_inflight_current: snapshot_named(&self.http_route_inflight_current),
            http_rejected_total: self.http_rejected_total.load(Ordering::Relaxed),
            http_rejected_global_total: self.http_rejected_global_total.load(Ordering::Relaxed),
            http_rejected_listener_total: self.http_rejected_listener_total.load(Ordering::Relaxed),
            http_rejected_route_total: self.http_rejected_route_total.load(Ordering::Relaxed),
            http_rejected_listener_by_name: snapshot_named(&self.http_rejected_listener_by_name),
            http_rejected_route_by_name: snapshot_named(&self.http_rejected_route_by_name),
            tcp_global_connections_current: self
                .tcp_global_connections_current
                .load(Ordering::Relaxed),
            tcp_listener_connections_current: snapshot_named(
                &self.tcp_listener_connections_current,
            ),
            tcp_rejected_total: self.tcp_rejected_total.load(Ordering::Relaxed),
            tcp_rejected_global_total: self.tcp_rejected_global_total.load(Ordering::Relaxed),
            tcp_rejected_listener_total: self.tcp_rejected_listener_total.load(Ordering::Relaxed),
            tcp_rejected_listener_by_name: snapshot_named(&self.tcp_rejected_listener_by_name),
            udp_global_datagrams_current: self.udp_global_datagrams_current.load(Ordering::Relaxed),
            udp_listener_datagrams_current: snapshot_named(&self.udp_listener_datagrams_current),
            udp_rejected_total: self.udp_rejected_total.load(Ordering::Relaxed),
            udp_rejected_global_total: self.udp_rejected_global_total.load(Ordering::Relaxed),
            udp_rejected_listener_total: self.udp_rejected_listener_total.load(Ordering::Relaxed),
            udp_rejected_listener_by_name: snapshot_named(&self.udp_rejected_listener_by_name),
        }
    }

    pub(super) fn observe_acquire(&self, scope: &BudgetScope) {
        match scope {
            BudgetScope::HttpGlobal => {
                self.http_global_inflight_current
                    .fetch_add(1, Ordering::Relaxed);
            }
            BudgetScope::HttpListener(listener) => {
                increment_named(&self.http_listener_inflight_current, listener.as_str())
            }
            BudgetScope::HttpRoute(route) => {
                increment_named(&self.http_route_inflight_current, route.as_str())
            }
            BudgetScope::TcpGlobal => {
                self.tcp_global_connections_current
                    .fetch_add(1, Ordering::Relaxed);
            }
            BudgetScope::TcpListener(listener) => {
                increment_named(&self.tcp_listener_connections_current, listener.as_str())
            }
            BudgetScope::UdpGlobal => {
                self.udp_global_datagrams_current
                    .fetch_add(1, Ordering::Relaxed);
            }
            BudgetScope::UdpListener(listener) => {
                increment_named(&self.udp_listener_datagrams_current, listener.as_str())
            }
        }
    }

    pub(super) fn observe_release(&self, scope: &BudgetScope) {
        match scope {
            BudgetScope::HttpGlobal => {
                saturating_decrement(&self.http_global_inflight_current);
            }
            BudgetScope::HttpListener(listener) => {
                decrement_named(&self.http_listener_inflight_current, listener.as_str())
            }
            BudgetScope::HttpRoute(route) => {
                decrement_named(&self.http_route_inflight_current, route.as_str())
            }
            BudgetScope::TcpGlobal => {
                saturating_decrement(&self.tcp_global_connections_current);
            }
            BudgetScope::TcpListener(listener) => {
                decrement_named(&self.tcp_listener_connections_current, listener.as_str())
            }
            BudgetScope::UdpGlobal => {
                saturating_decrement(&self.udp_global_datagrams_current);
            }
            BudgetScope::UdpListener(listener) => {
                decrement_named(&self.udp_listener_datagrams_current, listener.as_str())
            }
        }
    }

    pub(super) fn observe_reject(&self, scope: &BudgetScope) {
        match scope {
            BudgetScope::HttpGlobal => {
                self.http_rejected_total.fetch_add(1, Ordering::Relaxed);
                self.http_rejected_global_total
                    .fetch_add(1, Ordering::Relaxed);
            }
            BudgetScope::HttpListener(listener) => {
                self.http_rejected_total.fetch_add(1, Ordering::Relaxed);
                self.http_rejected_listener_total
                    .fetch_add(1, Ordering::Relaxed);
                increment_named(&self.http_rejected_listener_by_name, listener.as_str());
            }
            BudgetScope::HttpRoute(route) => {
                self.http_rejected_total.fetch_add(1, Ordering::Relaxed);
                self.http_rejected_route_total
                    .fetch_add(1, Ordering::Relaxed);
                increment_named(&self.http_rejected_route_by_name, route.as_str());
            }
            BudgetScope::TcpGlobal => {
                self.tcp_rejected_total.fetch_add(1, Ordering::Relaxed);
                self.tcp_rejected_global_total
                    .fetch_add(1, Ordering::Relaxed);
            }
            BudgetScope::TcpListener(listener) => {
                self.tcp_rejected_total.fetch_add(1, Ordering::Relaxed);
                self.tcp_rejected_listener_total
                    .fetch_add(1, Ordering::Relaxed);
                increment_named(&self.tcp_rejected_listener_by_name, listener.as_str());
            }
            BudgetScope::UdpGlobal => {
                self.udp_rejected_total.fetch_add(1, Ordering::Relaxed);
                self.udp_rejected_global_total
                    .fetch_add(1, Ordering::Relaxed);
            }
            BudgetScope::UdpListener(listener) => {
                self.udp_rejected_total.fetch_add(1, Ordering::Relaxed);
                self.udp_rejected_listener_total
                    .fetch_add(1, Ordering::Relaxed);
                increment_named(&self.udp_rejected_listener_by_name, listener.as_str());
            }
        }
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
