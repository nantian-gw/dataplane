use std::hash::{Hash, Hasher};
use std::sync::Arc;

use dashmap::DashMap;
use std::time::{Duration, Instant};

use crate::format::ir::{AIContent, AIMessage, AIRequest, AIResponse, AIRole};

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
        if self.max_entries == 0 {
            return;
        }

        // Evict expired entries first, then if still at capacity evict one more.
        if self.entries.len() >= self.max_entries {
            self.entries.retain(|_, v| !v.is_expired());
            if self.entries.len() >= self.max_entries {
                self.remove_eviction_candidate();
            }
        }
        self.entries.insert(key.to_string(), response.clone());
    }

    fn lookup(&self, key: &str) -> Option<CachedResponse> {
        let entry = self.entries.get(key).map(|r| r.clone())?;
        if entry.is_expired() {
            self.entries.remove(key);
            None
        } else {
            Some(entry)
        }
    }

    fn remove(&self, key: &str) {
        self.entries.remove(key);
    }
}

impl MemoryCacheBackend {
    fn remove_eviction_candidate(&self) {
        let evict_key = self.entries.iter().next().map(|e| e.key().clone());
        if let Some(key) = evict_key {
            self.entries.remove(&key);
        }
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

/// Build a cache key from the request model and the full conversation context.
/// Hashes the entire message history (system prompts, tool results, prior turns)
/// so that distinct conversations with different context but the same last user
/// message produce different cache keys.
pub fn build_cache_key(request: &AIRequest) -> String {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();

    request.model.hash(&mut hasher);
    hash_f32_option(request.temperature, &mut hasher);
    request.max_tokens.hash(&mut hasher);
    hash_f32_option(request.top_p, &mut hasher);
    request.stop.hash(&mut hasher);
    request.stream.hash(&mut hasher);
    request.user.hash(&mut hasher);
    if !request.extra.is_empty() {
        serde_json::to_string(&request.extra)
            .unwrap_or_default()
            .hash(&mut hasher);
    }

    // Include the full message history in order so that context and ordering
    // affect the key. This prevents cache collisions between conversations
    // that share only the final user message.
    for message in &request.messages {
        hash_message(message, &mut hasher);
    }

    format!("cache:{:016x}", hasher.finish())
}

fn hash_f32_option(value: Option<f32>, hasher: &mut impl Hasher) {
    value.map(f32::to_bits).hash(hasher);
}

fn hash_message(message: &AIMessage, hasher: &mut impl Hasher) {
    hash_role(message.role, hasher);
    message.name.hash(hasher);
    hash_content(&message.content, hasher);
    message.tool_call_id.hash(hasher);
    message.tool_calls.len().hash(hasher);
    for tool_call in &message.tool_calls {
        tool_call.id.hash(hasher);
        tool_call.call_type.hash(hasher);
        tool_call.function.name.hash(hasher);
        tool_call.function.arguments.hash(hasher);
    }
}

fn hash_role(role: AIRole, hasher: &mut impl Hasher) {
    match role {
        AIRole::System => 0_u8,
        AIRole::User => 1,
        AIRole::Assistant => 2,
        AIRole::Tool => 3,
    }
    .hash(hasher);
}

fn hash_content(content: &AIContent, hasher: &mut impl Hasher) {
    match content {
        AIContent::Text(text) => {
            0_u8.hash(hasher);
            text.hash(hasher);
        }
        AIContent::MultiPart(parts) => {
            1_u8.hash(hasher);
            parts.len().hash(hasher);
            for part in parts {
                part.content_type.hash(hasher);
                part.text.hash(hasher);
                if let Some(image_url) = &part.image_url {
                    image_url.url.hash(hasher);
                    image_url.detail.hash(hasher);
                }
            }
        }
        AIContent::None => {
            2_u8.hash(hasher);
        }
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
    #[must_use]
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

    /// Store a response in the cache with a pre-computed key.
    pub fn store(&self, key: &str, response: &AIResponse) {
        if !self.config.enabled {
            return;
        }
        let entry = CachedResponse {
            response: response.clone(),
            stored_at: Instant::now(),
            ttl: self.config.ttl,
        };
        self.backend.store(key, &entry, self.config.ttl);
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
    fn test_cache_eviction_recomputes_candidate_after_expiry_prune() {
        let backend = MemoryCacheBackend::with_capacity(2);
        let expired = CachedResponse {
            response: make_response("expired"),
            stored_at: Instant::now() - Duration::from_secs(120),
            ttl: Duration::from_secs(1),
        };
        let fresh = CachedResponse {
            response: make_response("fresh"),
            stored_at: Instant::now(),
            ttl: Duration::from_secs(60),
        };

        backend.entries.insert("expired".to_string(), expired);
        backend.entries.insert("fresh1".to_string(), fresh.clone());
        backend.entries.insert("fresh2".to_string(), fresh.clone());

        backend.store("fresh3", &fresh, Duration::from_secs(60));

        assert!(backend.lookup("expired").is_none());
        assert!(backend.lookup("fresh3").is_some());

        let live_entries = ["fresh1", "fresh2", "fresh3"]
            .into_iter()
            .filter(|key| backend.lookup(key).is_some())
            .count();
        assert_eq!(live_entries, 2, "cache must remain within capacity");
    }

    #[test]
    fn test_zero_capacity_does_not_store_entries() {
        let backend = MemoryCacheBackend::with_capacity(0);
        let resp = CachedResponse {
            response: make_response("r1"),
            stored_at: Instant::now(),
            ttl: Duration::from_secs(60),
        };

        backend.store("key1", &resp, Duration::from_secs(60));

        assert!(backend.lookup("key1").is_none());
    }

    #[test]
    fn test_lookup_removes_expired_entry() {
        let backend = MemoryCacheBackend::with_capacity(2);
        let expired = CachedResponse {
            response: make_response("expired"),
            stored_at: Instant::now() - Duration::from_secs(120),
            ttl: Duration::from_secs(1),
        };

        backend.entries.insert("expired".to_string(), expired);

        assert!(backend.lookup("expired").is_none());
        assert!(
            backend.entries.get("expired").is_none(),
            "expired cache entries should be removed on lookup"
        );
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
