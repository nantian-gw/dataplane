use std::{collections::BTreeMap, sync::Arc, time::Instant};

use parking_lot::RwLock;
use serde::Serialize;

mod bucket;
mod stats;

use self::{
    bucket::{
        KeyedScopeController, ScopeOptions, SharedBucket, new_shared_bucket, refund_bucket,
        snapshot_bucket, try_consume_bucket,
    },
    stats::HttpRateLimitStats,
};

pub type SharedHttpRateLimitController = Arc<HttpRateLimitController>;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct HttpRateLimitOptions {
    pub global_requests_per_second: u32,
    pub global_burst: u32,
    pub listener_requests_per_second: u32,
    pub listener_burst: u32,
    pub route_requests_per_second: u32,
    pub route_burst: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HttpRateLimitRejection {
    Global,
    Listener,
    Route,
}

impl HttpRateLimitRejection {
    pub fn scope_label(&self) -> &'static str {
        match self {
            Self::Global => "global",
            Self::Listener => "listener",
            Self::Route => "route",
        }
    }
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RateLimitScopeSnapshot {
    pub enabled: bool,
    pub requests_per_second: u32,
    pub burst: u32,
    pub available_tokens: u64,
    pub available_milli_tokens: u64,
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NamedRateLimitScopeSnapshot {
    pub enabled: bool,
    pub requests_per_second: u32,
    pub burst: u32,
    pub available_tokens_by_name: BTreeMap<String, u64>,
    pub available_milli_tokens_by_name: BTreeMap<String, u64>,
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HttpRateLimitSnapshot {
    pub global: RateLimitScopeSnapshot,
    pub listener: NamedRateLimitScopeSnapshot,
    pub route: NamedRateLimitScopeSnapshot,
    pub allowed_total: u64,
    pub rejected_total: u64,
    pub rejected_global_total: u64,
    pub rejected_listener_total: u64,
    pub rejected_route_total: u64,
    pub rejected_listener_by_name: BTreeMap<String, u64>,
    pub rejected_route_by_name: BTreeMap<String, u64>,
}

#[derive(Debug, Clone)]
pub struct HttpRateLimitController {
    global_options: ScopeOptions,
    global: Option<SharedBucket>,
    listener: KeyedScopeController,
    route: KeyedScopeController,
    stats: Arc<RwLock<HttpRateLimitStats>>,
}

impl HttpRateLimitController {
    pub fn new(options: HttpRateLimitOptions) -> Self {
        let global_options =
            ScopeOptions::new(options.global_requests_per_second, options.global_burst);
        Self {
            global_options,
            global: global_options
                .enabled()
                .then(|| new_shared_bucket(global_options)),
            listener: KeyedScopeController::new(
                options.listener_requests_per_second,
                options.listener_burst,
            ),
            route: KeyedScopeController::new(
                options.route_requests_per_second,
                options.route_burst,
            ),
            stats: Arc::new(RwLock::new(HttpRateLimitStats::default())),
        }
    }

    pub fn shared(options: HttpRateLimitOptions) -> SharedHttpRateLimitController {
        Arc::new(Self::new(options))
    }

    pub fn try_acquire_listener(&self, listener: &str) -> Result<bool, HttpRateLimitRejection> {
        let now = Instant::now();
        let global = self.global.clone();
        if let Some(bucket) = global.as_ref()
            && !try_consume_bucket(bucket, self.global_options, now)
        {
            self.observe_reject(HttpRateLimitRejection::Global, None);
            return Err(HttpRateLimitRejection::Global);
        }

        let listener_bucket = self.listener.bucket(listener);
        if let Some(bucket) = listener_bucket.as_ref()
            && !try_consume_bucket(bucket, self.listener.options, now)
        {
            if let Some(global_bucket) = global.as_ref() {
                refund_bucket(global_bucket, self.global_options, now);
            }
            self.observe_reject(HttpRateLimitRejection::Listener, Some(listener));
            return Err(HttpRateLimitRejection::Listener);
        }

        Ok(global.is_some() || listener_bucket.is_some())
    }

    pub fn try_acquire_route(&self, route: &str) -> Result<bool, HttpRateLimitRejection> {
        let now = Instant::now();
        let route_bucket = self.route.bucket(route);
        if let Some(bucket) = route_bucket.as_ref()
            && !try_consume_bucket(bucket, self.route.options, now)
        {
            self.observe_reject(HttpRateLimitRejection::Route, Some(route));
            return Err(HttpRateLimitRejection::Route);
        }

        Ok(route_bucket.is_some())
    }

    pub fn route_scope_enabled(&self) -> bool {
        self.route.options.enabled()
    }

    pub fn try_acquire(&self, listener: &str, route: &str) -> Result<bool, HttpRateLimitRejection> {
        let now = Instant::now();
        let global = self.global.clone();
        if let Some(bucket) = global.as_ref()
            && !try_consume_bucket(bucket, self.global_options, now)
        {
            self.observe_reject(HttpRateLimitRejection::Global, None);
            return Err(HttpRateLimitRejection::Global);
        }

        let listener_bucket = self.listener.bucket(listener);
        if let Some(bucket) = listener_bucket.as_ref()
            && !try_consume_bucket(bucket, self.listener.options, now)
        {
            if let Some(global_bucket) = global.as_ref() {
                refund_bucket(global_bucket, self.global_options, now);
            }
            self.observe_reject(HttpRateLimitRejection::Listener, Some(listener));
            return Err(HttpRateLimitRejection::Listener);
        }

        let route_bucket = self.route.bucket(route);
        if let Some(bucket) = route_bucket.as_ref()
            && !try_consume_bucket(bucket, self.route.options, now)
        {
            if let Some(listener_bucket) = listener_bucket.as_ref() {
                refund_bucket(listener_bucket, self.listener.options, now);
            }
            if let Some(global_bucket) = global.as_ref() {
                refund_bucket(global_bucket, self.global_options, now);
            }
            self.observe_reject(HttpRateLimitRejection::Route, Some(route));
            return Err(HttpRateLimitRejection::Route);
        }

        Ok(global.is_some() || listener_bucket.is_some() || route_bucket.is_some())
    }

    pub fn snapshot(&self) -> HttpRateLimitSnapshot {
        let now = Instant::now();
        let global = self
            .global
            .as_ref()
            .map(|bucket| snapshot_bucket(bucket, self.global_options, now))
            .unwrap_or_else(|| self.global_options.snapshot(0));
        let stats = self.stats.read();

        HttpRateLimitSnapshot {
            global,
            listener: self.listener.snapshot(now),
            route: self.route.snapshot(now),
            allowed_total: stats.allowed_total,
            rejected_total: stats.rejected_total,
            rejected_global_total: stats.rejected_global_total,
            rejected_listener_total: stats.rejected_listener_total,
            rejected_route_total: stats.rejected_route_total,
            rejected_listener_by_name: stats.rejected_listener_by_name.clone(),
            rejected_route_by_name: stats.rejected_route_by_name.clone(),
        }
    }

    pub fn observe_allow(&self) {
        if !self.global_options.enabled()
            && !self.listener.options.enabled()
            && !self.route.options.enabled()
        {
            return;
        }

        let mut stats = self.stats.write();
        stats.observe_allow();
    }

    fn observe_reject(&self, rejection: HttpRateLimitRejection, key: Option<&str>) {
        let mut stats = self.stats.write();
        stats.observe_reject(rejection, key);
    }
}

#[cfg(test)]
mod tests;
