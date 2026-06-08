use std::sync::Arc;

use tokio::sync::{OwnedSemaphorePermit, Semaphore};

use super::{SemaphoreMap, SharedOverloadStats};

#[derive(Debug, Clone)]
pub(super) enum BudgetScope {
    HttpGlobal,
    HttpListener(String),
    HttpRoute(String),
    TcpGlobal,
    TcpListener(String),
    UdpGlobal,
    UdpListener(String),
}

#[derive(Debug)]
pub(super) struct TrackedPermit {
    permit: Option<OwnedSemaphorePermit>,
    scope: BudgetScope,
    stats: SharedOverloadStats,
    cleanup: Option<KeyedCleanup>,
}

#[derive(Debug)]
struct KeyedCleanup {
    entries: SemaphoreMap,
    key: String,
    limit: usize,
}

impl Drop for TrackedPermit {
    fn drop(&mut self) {
        self.stats.observe_release(&self.scope);
        if let Some(permit) = self.permit.take() {
            drop(permit);
        }
        if let Some(cleanup) = &self.cleanup {
            let mut entries = cleanup
                .entries
                .write()
                .unwrap_or_else(|err| err.into_inner());
            let remove = entries.get(&cleanup.key).is_some_and(|semaphore| {
                Arc::strong_count(semaphore) == 1 && semaphore.available_permits() == cleanup.limit
            });
            if remove {
                entries.remove(cleanup.key.as_str());
            }
        }
    }
}

pub(super) fn semaphore_for_limit(limit: usize) -> Option<Arc<Semaphore>> {
    (limit > 0).then(|| Arc::new(Semaphore::new(limit)))
}

pub(super) fn try_acquire_scope<E: Copy>(
    semaphore: Option<Arc<Semaphore>>,
    stats: &SharedOverloadStats,
    scope: BudgetScope,
    rejection: E,
) -> Result<Option<TrackedPermit>, E> {
    let Some(semaphore) = semaphore else {
        return Ok(None);
    };

    match semaphore.try_acquire_owned() {
        Ok(permit) => {
            stats.observe_acquire(&scope);
            Ok(Some(TrackedPermit {
                permit: Some(permit),
                scope,
                stats: stats.clone(),
                cleanup: None,
            }))
        }
        Err(_) => {
            stats.observe_reject(&scope);
            Err(rejection)
        }
    }
}

pub(super) fn try_acquire_keyed_scope<E: Copy, F: FnOnce(&str) -> BudgetScope>(
    entries: &SemaphoreMap,
    limit: usize,
    key: &str,
    stats: &SharedOverloadStats,
    scope_builder: F,
    rejection: E,
) -> Result<Option<TrackedPermit>, E> {
    let key = key.trim();
    if limit == 0 || key.is_empty() {
        return Ok(None);
    }
    let semaphore = keyed_semaphore(entries, key, limit);
    let scope = scope_builder(key);

    match semaphore.clone().try_acquire_owned() {
        Ok(permit) => {
            stats.observe_acquire(&scope);
            Ok(Some(TrackedPermit {
                permit: Some(permit),
                scope,
                stats: stats.clone(),
                cleanup: Some(KeyedCleanup {
                    entries: Arc::clone(entries),
                    key: key.to_string(),
                    limit,
                }),
            }))
        }
        Err(_) => {
            stats.observe_reject(&scope);
            Err(rejection)
        }
    }
}

pub(super) fn keyed_semaphore(entries: &SemaphoreMap, key: &str, limit: usize) -> Arc<Semaphore> {
    if let Some(semaphore) = entries
        .read()
        .unwrap_or_else(|err| err.into_inner())
        .get(key)
        .cloned()
    {
        return semaphore;
    }

    let mut entries = entries.write().unwrap_or_else(|err| err.into_inner());
    entries
        .entry(key.to_string())
        .or_insert_with(|| Arc::new(Semaphore::new(limit)))
        .clone()
}
