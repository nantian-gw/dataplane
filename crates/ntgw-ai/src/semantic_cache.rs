use std::borrow::Cow;
use std::hash::{Hash, Hasher};
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

/// Default maximum number of cache entries before eviction kicks in.
pub const DEFAULT_MAX_ENTRIES: usize = 10_000;

/// Simple in-memory cache backend using `DashMap` for lock-free concurrent access
/// with capacity-bounded eviction.
pub struct MemoryCacheBackend {
    entries: DashMap<String, CachedResponse>,
    max_entries: usize,
}

impl MemoryCacheBackend {
    pub fn new() -> Self {
        Self {
            entries: DashMap::new(),
            max_entries: DEFAULT_MAX_ENTRIES,
        }
    }

    /// Create a backend with a custom entry cap.
    pub fn with_capacity(max_entries: usize) -> Self {
        Self {
            entries: DashMap::new(),
            max_entries,
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
        // Evict expired entries first, then if still at capacity evict one more.
        if self.entries.len() >= self.max_entries {
            self.entries.retain(|_, v| !v.is_expired());
            let evict_key = self.entries.iter().next().map(|e| e.key().clone());
            #[allow(clippy::collapsible_if)]
            if self.entries.len() >= self.max_entries {
                if let Some(key) = evict_key {
                    self.entries.remove(&key);
                }
            }
        }
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

/// Build a cache key from the request model and a hash of the last user message content.
/// Uses a full-content hash instead of a truncated prefix to avoid collisions.
pub fn build_cache_key(request: &AIRequest) -> String {
    let content = request
        .messages
        .iter()
        .rev()
        .find(|m| matches!(m.role, AIRole::User))
        .map(|m| content_str(&m.content))
        .unwrap_or_default();

    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    request.model.hash(&mut hasher);
    content.hash(&mut hasher);
    format!("cache:{:016x}", hasher.finish())
}

fn content_str(content: &AIContent) -> Cow<'_, str> {
    match content {
        AIContent::Text(s) => Cow::Borrowed(s.as_str()),
        AIContent::MultiPart(parts) => {
            let texts: Vec<&str> = parts.iter().filter_map(|p| p.text.as_deref()).collect();
            Cow::Owned(texts.join(" "))
        }
        AIContent::None => Cow::Borrowed(""),
    }
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
        if let Some(entry) = self.backend.lookup(&key)
            && !entry.is_expired()
        {
            return Some(entry.response);
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

#[cfg(test)]
mod tests {
    use super::*;

    fn make_response(id: &str) -> AIResponse {
        AIResponse {
            id: id.to_string(),
            model: "test".to_string(),
            choices: vec![],
            usage: None,
            created: None,
            extra: Default::default(),
        }
    }

    #[test]
    fn test_cache_eviction_at_capacity() {
        let backend = MemoryCacheBackend::with_capacity(2);
        let resp = CachedResponse {
            response: make_response("r1"),
            stored_at: Instant::now(),
            ttl: Duration::from_secs(60),
        };
        backend.store("key1", &resp, Duration::from_secs(60));
        backend.store("key2", &resp, Duration::from_secs(60));
        backend.store("key3", &resp, Duration::from_secs(60));
        // After inserting key3 at capacity 2, at most one of key1/key2 should remain
        let present =
            backend.lookup("key1").is_some() as u8 + backend.lookup("key2").is_some() as u8;
        assert!(
            present <= 1,
            "at most one of key1/key2 should remain after eviction"
        );
        assert!(backend.lookup("key3").is_some(), "key3 must be present");
    }

    #[test]
    fn test_build_cache_key_different_content_produces_different_keys() {
        let req1 = AIRequest {
            messages: vec![],
            model: "gpt-4".into(),
            temperature: None,
            max_tokens: None,
            top_p: None,
            stop: vec![],
            stream: false,
            user: None,
            extra: Default::default(),
        };
        let req2 = AIRequest {
            messages: vec![],
            model: "gpt-4o".into(),
            temperature: None,
            max_tokens: None,
            top_p: None,
            stop: vec![],
            stream: false,
            user: None,
            extra: Default::default(),
        };
        assert_ne!(build_cache_key(&req1), build_cache_key(&req2));
    }
}
