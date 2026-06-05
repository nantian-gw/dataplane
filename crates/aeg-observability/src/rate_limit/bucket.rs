use std::{
    collections::BTreeMap,
    sync::Arc,
    time::{Duration, Instant},
};

use parking_lot::{Mutex, RwLock};

use super::{NamedRateLimitScopeSnapshot, RateLimitScopeSnapshot};

pub(super) const RATE_LIMIT_TOKEN_SCALE: u64 = 1_000;

pub(super) type SharedBucket = Arc<Mutex<RateLimitBucket>>;
type BucketMap = Arc<RwLock<BTreeMap<String, SharedBucket>>>;

#[derive(Debug, Clone, Copy)]
pub(super) struct ScopeOptions {
    pub(super) requests_per_second: u32,
    burst: u32,
}

#[derive(Debug, Clone)]
pub(super) struct KeyedScopeController {
    pub(super) options: ScopeOptions,
    buckets: BucketMap,
}

#[derive(Debug)]
pub(super) struct RateLimitBucket {
    pub(super) available_milli_tokens: u64,
    last_refill: Instant,
}

impl ScopeOptions {
    pub(super) fn new(requests_per_second: u32, burst: u32) -> Self {
        Self {
            requests_per_second,
            burst,
        }
    }

    pub(super) fn enabled(self) -> bool {
        self.requests_per_second > 0
    }

    pub(super) fn burst(self) -> u32 {
        if self.enabled() {
            self.burst.max(1)
        } else {
            0
        }
    }

    pub(super) fn capacity(self) -> u64 {
        if !self.enabled() {
            return 0;
        }

        u64::from(self.burst()) * RATE_LIMIT_TOKEN_SCALE
    }

    fn refill(self, elapsed: Duration) -> u64 {
        if !self.enabled() {
            return 0;
        }

        let elapsed_millis = elapsed.as_millis().min(u128::from(u64::MAX)) as u64;
        elapsed_millis
            .saturating_mul(u64::from(self.requests_per_second))
            .saturating_mul(RATE_LIMIT_TOKEN_SCALE)
            / 1_000
    }

    pub(super) fn snapshot(self, available_milli_tokens: u64) -> RateLimitScopeSnapshot {
        RateLimitScopeSnapshot {
            enabled: self.enabled(),
            requests_per_second: self.requests_per_second,
            burst: self.burst(),
            available_tokens: available_milli_tokens / RATE_LIMIT_TOKEN_SCALE,
            available_milli_tokens,
        }
    }
}

impl Default for ScopeOptions {
    fn default() -> Self {
        Self::new(0, 0)
    }
}

impl KeyedScopeController {
    pub(super) fn new(requests_per_second: u32, burst: u32) -> Self {
        Self {
            options: ScopeOptions::new(requests_per_second, burst),
            buckets: Arc::new(RwLock::new(BTreeMap::new())),
        }
    }

    pub(super) fn bucket(&self, key: &str) -> Option<SharedBucket> {
        if !self.options.enabled() {
            return None;
        }

        let key = key.trim();
        if key.is_empty() {
            return None;
        }

        if let Some(bucket) = self.buckets.read().get(key).cloned() {
            return Some(bucket);
        }

        let mut buckets = self.buckets.write();
        Some(
            buckets
                .entry(key.to_string())
                .or_insert_with(|| Arc::new(Mutex::new(new_bucket(self.options))))
                .clone(),
        )
    }

    pub(super) fn snapshot(&self, now: Instant) -> NamedRateLimitScopeSnapshot {
        let mut available_tokens_by_name = BTreeMap::new();
        let mut available_milli_tokens_by_name = BTreeMap::new();
        let buckets = self.buckets.read();
        for (name, bucket) in buckets.iter() {
            let mut state = bucket.lock();
            refill_bucket(&mut state, self.options, now);
            available_tokens_by_name.insert(
                name.clone(),
                state.available_milli_tokens / RATE_LIMIT_TOKEN_SCALE,
            );
            available_milli_tokens_by_name.insert(name.clone(), state.available_milli_tokens);
        }

        NamedRateLimitScopeSnapshot {
            enabled: self.options.enabled(),
            requests_per_second: self.options.requests_per_second,
            burst: self.options.burst(),
            available_tokens_by_name,
            available_milli_tokens_by_name,
        }
    }
}

fn new_bucket(options: ScopeOptions) -> RateLimitBucket {
    RateLimitBucket {
        available_milli_tokens: options.capacity(),
        last_refill: Instant::now(),
    }
}

pub(super) fn new_shared_bucket(options: ScopeOptions) -> SharedBucket {
    Arc::new(Mutex::new(new_bucket(options)))
}

pub(super) fn snapshot_bucket(
    bucket: &SharedBucket,
    options: ScopeOptions,
    now: Instant,
) -> RateLimitScopeSnapshot {
    let mut state = bucket.lock();
    refill_bucket(&mut state, options, now);
    options.snapshot(state.available_milli_tokens)
}

pub(super) fn refill_bucket(state: &mut RateLimitBucket, options: ScopeOptions, now: Instant) {
    if !options.enabled() {
        state.available_milli_tokens = 0;
        state.last_refill = now;
        return;
    }

    let refill = options.refill(now.saturating_duration_since(state.last_refill));
    if refill > 0 {
        state.available_milli_tokens = state
            .available_milli_tokens
            .saturating_add(refill)
            .min(options.capacity());
    }
    state.last_refill = now;
}

pub(super) fn try_consume_bucket(
    bucket: &SharedBucket,
    options: ScopeOptions,
    now: Instant,
) -> bool {
    let mut state = bucket.lock();
    refill_bucket(&mut state, options, now);
    if state.available_milli_tokens < RATE_LIMIT_TOKEN_SCALE {
        return false;
    }

    state.available_milli_tokens -= RATE_LIMIT_TOKEN_SCALE;
    true
}

pub(super) fn refund_bucket(bucket: &SharedBucket, options: ScopeOptions, now: Instant) {
    let mut state = bucket.lock();
    refill_bucket(&mut state, options, now);
    state.available_milli_tokens = state
        .available_milli_tokens
        .saturating_add(RATE_LIMIT_TOKEN_SCALE)
        .min(options.capacity());
}
