use std::sync::Arc;

use dashmap::DashMap;
use std::time::{Duration, Instant};

use crate::format::ir::{AIContent, AIRequest, AIResponse, AIRole};

/// Backend trait for cache storage. Implementations: memory, Redis, pgvector.
pub trait CacheBackend: Send + Sync {
    fn store(&self, key: &str, response: &CachedResponse, ttl: Duration);
    fn lookup(&self, key: &str) -> Option<CachedResponse>;
    fn remove(&self, key: &str);
}

/// A cached AI response with timestamp and TTL for expiration.
#[derive(Debug, Clone)]
pub struct CachedResponse {
    pub response: AIResponse,
    pub stored_at: Instant,
    pub ttl: Duration,
}

impl CachedResponse {
    pub fn is_expired(&self) -> bool {
        self.stored_at.elapsed() > self.ttl
    }
}

/// Simple in-memory cache backend using `DashMap` for lock-free concurrent access.
pub struct MemoryCacheBackend {
    entries: DashMap<String, CachedResponse>,
}

impl MemoryCacheBackend {
    pub fn new() -> Self {
        Self {
            entries: DashMap::new(),
        }
    }
}

impl Default for MemoryCacheBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl CacheBackend for MemoryCacheBackend {
    fn store(&self, key: &str, response: &CachedResponse, _ttl: Duration) {
        self.entries.insert(key.to_string(), response.clone());
    }

    fn lookup(&self, key: &str) -> Option<CachedResponse> {
        self.entries
            .get(key)
            .map(|r| r.clone())
            .filter(|c| !c.is_expired())
    }

    fn remove(&self, key: &str) {
        self.entries.remove(key);
    }
}

/// Configuration for semantic cache.
#[derive(Debug, Clone)]
pub struct CacheConfig {
    pub enabled: bool,
    pub ttl: Duration,
    pub max_tokens: usize,
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            ttl: Duration::from_secs(3600),
            max_tokens: 4096,
        }
    }
}

/// Build a cache key from the request's last user message and model.
fn build_cache_key(request: &AIRequest) -> String {
    let last_msg = request
        .messages
        .iter()
        .rev()
        .find(|m| matches!(m.role, AIRole::User));

    let content = match last_msg.map(|m| &m.content) {
        Some(AIContent::Text(s)) => s.clone(),
        Some(AIContent::MultiPart(parts)) => parts
            .iter()
            .filter_map(|p| p.text.clone())
            .collect::<Vec<_>>()
            .join(" "),
        Some(AIContent::None) | None => String::new(),
    };

    format!(
        "cache:{}:{}:{}",
        request.model,
        content.len(),
        content.chars().take(100).collect::<String>(),
    )
}

/// Semantic cache: lookup/store against a generic [`CacheBackend`].
pub struct SemanticCache {
    backend: Arc<dyn CacheBackend>,
    config: CacheConfig,
}

impl SemanticCache {
    pub fn new(backend: Arc<dyn CacheBackend>, config: CacheConfig) -> Self {
        Self { backend, config }
    }

    /// Convenience constructor using the in-memory backend.
    pub fn with_memory_backend(config: CacheConfig) -> Self {
        Self::new(Arc::new(MemoryCacheBackend::new()), config)
    }

    /// Try to find a cached response. Returns `None` if cache is disabled,
    /// no entry exists, or the entry is expired.
    pub fn lookup(&self, request: &AIRequest) -> Option<AIResponse> {
        if !self.config.enabled {
            return None;
        }
        let key = build_cache_key(request);
        if let Some(entry) = self.backend.lookup(&key) {
            if !entry.is_expired() {
                return Some(entry.response);
            }
        }
        None
    }

    /// Store a response in the cache.
    pub fn store(&self, request: &AIRequest, response: &AIResponse) {
        if !self.config.enabled {
            return;
        }
        let key = build_cache_key(request);
        let entry = CachedResponse {
            response: response.clone(),
            stored_at: Instant::now(),
            ttl: self.config.ttl,
        };
        self.backend.store(&key, &entry, self.config.ttl);
    }
}
