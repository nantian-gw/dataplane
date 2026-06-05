use std::{
    collections::BTreeMap,
    sync::{Arc, RwLock as StdRwLock},
};

use tokio::sync::Semaphore;

mod budget;
mod stats;
#[cfg(test)]
mod tests;

pub use self::stats::{OverloadSnapshot, OverloadStats, SharedOverloadStats};
use budget::{
    semaphore_for_limit, try_acquire_keyed_scope, try_acquire_scope, BudgetScope, TrackedPermit,
};

type SemaphoreMap = Arc<StdRwLock<BTreeMap<String, Arc<Semaphore>>>>;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct HttpAdmissionOptions {
    pub global_inflight_limit: usize,
    pub listener_inflight_limit: usize,
    pub route_inflight_limit: usize,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TcpAdmissionOptions {
    pub global_connection_limit: usize,
    pub listener_connection_limit: usize,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct UdpAdmissionOptions {
    pub global_datagram_limit: usize,
    pub listener_datagram_limit: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HttpAdmissionRejection {
    GlobalInflight,
    ListenerInflight,
    RouteInflight,
}

impl HttpAdmissionRejection {
    pub fn scope_label(&self) -> &'static str {
        match self {
            Self::GlobalInflight => "global",
            Self::ListenerInflight => "listener",
            Self::RouteInflight => "route",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TcpAdmissionRejection {
    GlobalConnection,
    ListenerConnection,
}

impl TcpAdmissionRejection {
    pub fn scope_label(&self) -> &'static str {
        match self {
            Self::GlobalConnection => "global",
            Self::ListenerConnection => "listener",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UdpAdmissionRejection {
    GlobalDatagram,
    ListenerDatagram,
}

impl UdpAdmissionRejection {
    pub fn scope_label(&self) -> &'static str {
        match self {
            Self::GlobalDatagram => "global",
            Self::ListenerDatagram => "listener",
        }
    }
}

#[derive(Debug, Clone)]
pub struct HttpAdmissionController {
    global: Option<Arc<Semaphore>>,
    listener_limit: usize,
    route_limit: usize,
    listeners: SemaphoreMap,
    routes: SemaphoreMap,
    stats: SharedOverloadStats,
}

#[derive(Debug, Clone)]
pub struct TcpAdmissionController {
    global: Option<Arc<Semaphore>>,
    listener_limit: usize,
    listeners: SemaphoreMap,
    stats: SharedOverloadStats,
}

#[derive(Debug, Clone)]
pub struct UdpAdmissionController {
    global: Option<Arc<Semaphore>>,
    listener_limit: usize,
    listeners: SemaphoreMap,
    stats: SharedOverloadStats,
}

#[derive(Debug)]
pub struct HttpAdmissionPermit {
    permits: Vec<TrackedPermit>,
}

#[derive(Debug)]
pub struct TcpAdmissionPermit {
    permits: Vec<TrackedPermit>,
}

#[derive(Debug)]
pub struct UdpAdmissionPermit {
    permits: Vec<TrackedPermit>,
}

impl HttpAdmissionController {
    pub fn new(options: HttpAdmissionOptions, stats: SharedOverloadStats) -> Self {
        Self {
            global: semaphore_for_limit(options.global_inflight_limit),
            listener_limit: options.listener_inflight_limit,
            route_limit: options.route_inflight_limit,
            listeners: Arc::new(StdRwLock::new(BTreeMap::new())),
            routes: Arc::new(StdRwLock::new(BTreeMap::new())),
            stats,
        }
    }

    pub fn try_acquire_listener(
        &self,
        listener: &str,
    ) -> Result<HttpAdmissionPermit, HttpAdmissionRejection> {
        let mut permits = Vec::with_capacity(2);
        if let Some(permit) = try_acquire_scope(
            self.global.clone(),
            &self.stats,
            BudgetScope::HttpGlobal,
            HttpAdmissionRejection::GlobalInflight,
        )? {
            permits.push(permit);
        }
        if let Some(permit) = try_acquire_keyed_scope(
            &self.listeners,
            self.listener_limit,
            listener,
            &self.stats,
            |key| BudgetScope::HttpListener(key.to_string()),
            HttpAdmissionRejection::ListenerInflight,
        )? {
            permits.push(permit);
        }
        Ok(HttpAdmissionPermit { permits })
    }

    pub fn try_acquire_route(
        &self,
        route: &str,
    ) -> Result<HttpAdmissionPermit, HttpAdmissionRejection> {
        let mut permits = Vec::with_capacity(1);
        if let Some(permit) = try_acquire_keyed_scope(
            &self.routes,
            self.route_limit,
            route,
            &self.stats,
            |key| BudgetScope::HttpRoute(key.to_string()),
            HttpAdmissionRejection::RouteInflight,
        )? {
            permits.push(permit);
        }
        Ok(HttpAdmissionPermit { permits })
    }

    pub fn route_scope_enabled(&self) -> bool {
        self.route_limit > 0
    }

    pub fn try_acquire(
        &self,
        listener: &str,
        route: &str,
    ) -> Result<HttpAdmissionPermit, HttpAdmissionRejection> {
        let mut permit = self.try_acquire_listener(listener)?;
        permit.merge(self.try_acquire_route(route)?);
        Ok(permit)
    }
}

impl TcpAdmissionController {
    pub fn new(options: TcpAdmissionOptions, stats: SharedOverloadStats) -> Self {
        Self {
            global: semaphore_for_limit(options.global_connection_limit),
            listener_limit: options.listener_connection_limit,
            listeners: Arc::new(StdRwLock::new(BTreeMap::new())),
            stats,
        }
    }

    pub fn try_acquire(&self, listener: &str) -> Result<TcpAdmissionPermit, TcpAdmissionRejection> {
        let mut permits = Vec::with_capacity(2);
        if let Some(permit) = try_acquire_scope(
            self.global.clone(),
            &self.stats,
            BudgetScope::TcpGlobal,
            TcpAdmissionRejection::GlobalConnection,
        )? {
            permits.push(permit);
        }
        if let Some(permit) = try_acquire_keyed_scope(
            &self.listeners,
            self.listener_limit,
            listener,
            &self.stats,
            |key| BudgetScope::TcpListener(key.to_string()),
            TcpAdmissionRejection::ListenerConnection,
        )? {
            permits.push(permit);
        }
        Ok(TcpAdmissionPermit { permits })
    }
}

impl UdpAdmissionController {
    pub fn new(options: UdpAdmissionOptions, stats: SharedOverloadStats) -> Self {
        Self {
            global: semaphore_for_limit(options.global_datagram_limit),
            listener_limit: options.listener_datagram_limit,
            listeners: Arc::new(StdRwLock::new(BTreeMap::new())),
            stats,
        }
    }

    pub fn try_acquire(&self, listener: &str) -> Result<UdpAdmissionPermit, UdpAdmissionRejection> {
        let mut permits = Vec::with_capacity(2);
        if let Some(permit) = try_acquire_scope(
            self.global.clone(),
            &self.stats,
            BudgetScope::UdpGlobal,
            UdpAdmissionRejection::GlobalDatagram,
        )? {
            permits.push(permit);
        }
        if let Some(permit) = try_acquire_keyed_scope(
            &self.listeners,
            self.listener_limit,
            listener,
            &self.stats,
            |key| BudgetScope::UdpListener(key.to_string()),
            UdpAdmissionRejection::ListenerDatagram,
        )? {
            permits.push(permit);
        }
        Ok(UdpAdmissionPermit { permits })
    }
}

impl HttpAdmissionPermit {
    pub fn active_budget_count(&self) -> usize {
        self.permits.len()
    }

    pub fn merge(&mut self, mut other: Self) {
        self.permits.append(&mut other.permits);
    }
}

impl TcpAdmissionPermit {
    pub fn active_budget_count(&self) -> usize {
        self.permits.len()
    }
}

impl UdpAdmissionPermit {
    pub fn active_budget_count(&self) -> usize {
        self.permits.len()
    }
}
