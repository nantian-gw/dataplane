use parking_lot::RwLock;
use std::{
    collections::{BTreeMap, HashMap},
    sync::Arc,
};

use tokio::sync::{OwnedSemaphorePermit, Semaphore};

#[cfg(test)]
mod tests;

type SemaphoreMap = Arc<RwLock<BTreeMap<String, Arc<Semaphore>>>>;

pub type SharedHttpCircuitBreakerController = Arc<HttpCircuitBreakerController>;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct HttpCircuitBreakerOptions {
    pub backend_max_inflight_requests: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HttpCircuitBreakerRejection {
    Backend,
}

impl HttpCircuitBreakerRejection {
    pub fn scope_label(&self) -> &'static str {
        match self {
            Self::Backend => "backend",
        }
    }
}

#[derive(Debug, Clone, Default, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HttpCircuitBreakerSnapshot {
    pub backend_max_inflight_requests: usize,
    pub backend_inflight_current: BTreeMap<String, u64>,
    pub rejected_total: u64,
    pub rejected_backend_total: u64,
    pub rejected_backend_by_name: BTreeMap<String, u64>,
}

#[derive(Debug, Default)]
pub struct HttpCircuitBreakerStats {
    state: RwLock<HttpCircuitBreakerSnapshot>,
}

#[derive(Debug, Clone)]
pub struct HttpCircuitBreakerController {
    backend_limit: usize,
    per_backend_limits: Arc<RwLock<HashMap<String, usize>>>,
    backends: SemaphoreMap,
    stats: Arc<HttpCircuitBreakerStats>,
}

#[derive(Debug)]
pub struct HttpCircuitBreakerPermit {
    permit: Option<OwnedSemaphorePermit>,
    backend: String,
    stats: Arc<HttpCircuitBreakerStats>,
    cleanup: Option<KeyedCleanup>,
}

#[derive(Debug)]
struct KeyedCleanup {
    entries: SemaphoreMap,
    key: String,
    limit: usize,
}

impl HttpCircuitBreakerController {
    pub fn new(options: HttpCircuitBreakerOptions) -> Self {
        Self {
            backend_limit: options.backend_max_inflight_requests,
            per_backend_limits: Arc::new(RwLock::new(HashMap::new())),
            backends: Arc::new(RwLock::new(BTreeMap::new())),
            stats: Arc::new(HttpCircuitBreakerStats {
                state: RwLock::new(HttpCircuitBreakerSnapshot {
                    backend_max_inflight_requests: options.backend_max_inflight_requests,
                    ..HttpCircuitBreakerSnapshot::default()
                }),
            }),
        }
    }

    pub fn shared(options: HttpCircuitBreakerOptions) -> SharedHttpCircuitBreakerController {
        Arc::new(Self::new(options))
    }

    pub fn try_acquire_backend(
        &self,
        backend: &str,
    ) -> Result<HttpCircuitBreakerPermit, HttpCircuitBreakerRejection> {
        let backend = backend.trim();
        let limit = self.backend_limit_for(backend);
        if limit == 0 || backend.is_empty() {
            return Ok(HttpCircuitBreakerPermit {
                permit: None,
                backend: backend.to_string(),
                stats: self.stats.clone(),
                cleanup: None,
            });
        }

        let semaphore = keyed_semaphore(&self.backends, backend, limit);
        match semaphore.clone().try_acquire_owned() {
            Ok(permit) => {
                self.stats.observe_backend_acquire(backend);
                Ok(HttpCircuitBreakerPermit {
                    permit: Some(permit),
                    backend: backend.to_string(),
                    stats: self.stats.clone(),
                    cleanup: Some(KeyedCleanup {
                        entries: self.backends.clone(),
                        key: backend.to_string(),
                        limit,
                    }),
                })
            }
            Err(_) => {
                self.stats.observe_backend_reject(backend);
                Err(HttpCircuitBreakerRejection::Backend)
            }
        }
    }

    fn backend_limit_for(&self, backend: &str) -> usize {
        self.per_backend_limits
            .read()
            .get(backend)
            .copied()
            .unwrap_or(self.backend_limit)
    }

    pub fn set_backend_limit(&self, backend: &str, limit: usize) {
        self.per_backend_limits
            .write()
            .insert(backend.to_string(), limit);
    }

    pub fn set_per_backend_limits(&self, limits: HashMap<String, usize>) {
        *self.per_backend_limits.write() = limits;
    }

    pub fn snapshot(&self) -> HttpCircuitBreakerSnapshot {
        self.stats.snapshot()
    }
}

impl HttpCircuitBreakerStats {
    fn snapshot(&self) -> HttpCircuitBreakerSnapshot {
        self.state.read().clone()
    }

    fn observe_backend_acquire(&self, backend: &str) {
        let mut state = self.state.write();
        increment_named(&mut state.backend_inflight_current, backend);
    }

    fn observe_backend_release(&self, backend: &str) {
        let mut state = self.state.write();
        decrement_named(&mut state.backend_inflight_current, backend);
    }

    fn observe_backend_reject(&self, backend: &str) {
        let mut state = self.state.write();
        state.rejected_total += 1;
        state.rejected_backend_total += 1;
        increment_named(&mut state.rejected_backend_by_name, backend);
    }
}

impl Drop for HttpCircuitBreakerPermit {
    fn drop(&mut self) {
        if self.permit.take().is_some() && !self.backend.is_empty() {
            self.stats.observe_backend_release(&self.backend);
        }

        if let Some(cleanup) = &self.cleanup {
            let mut entries = cleanup.entries.write();
            let remove = entries.get(cleanup.key.as_str()).is_some_and(|semaphore| {
                Arc::strong_count(semaphore) == 1 && semaphore.available_permits() == cleanup.limit
            });
            if remove {
                entries.remove(cleanup.key.as_str());
            }
        }
    }
}

fn keyed_semaphore(entries: &SemaphoreMap, key: &str, limit: usize) -> Arc<Semaphore> {
    if let Some(semaphore) = entries.read().get(key).cloned() {
        return semaphore;
    }

    let mut entries = entries.write();
    entries
        .entry(key.to_string())
        .or_insert_with(|| Arc::new(Semaphore::new(limit)))
        .clone()
}

fn increment_named(items: &mut BTreeMap<String, u64>, key: &str) {
    *items.entry(key.to_string()).or_default() += 1;
}

fn decrement_named(items: &mut BTreeMap<String, u64>, key: &str) {
    let Some(value) = items.get_mut(key) else {
        return;
    };
    *value = value.saturating_sub(1);
    if *value == 0 {
        items.remove(key);
    }
}
