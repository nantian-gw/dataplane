use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use pingora_cache::cache_control::CacheControl;
use pingora_cache::eviction::simple_lru::Manager as LruManager;
use pingora_cache::filters::{request_cacheable, resp_cacheable};
use pingora_cache::key::CacheKey;
use pingora_cache::lock::{CacheKeyLockImpl, CacheLock};
use pingora_cache::meta::CacheMetaDefaults;
use pingora_cache::storage::Storage;
use pingora_cache::{CacheMeta, HttpCache, MemCache, RespCacheable};

use ntgw_config::HttpCacheConfig;

#[derive(Clone)]
pub struct CacheOptions {
    pub enabled: bool,
    pub max_size_bytes: usize,
    pub default_ttl: Duration,
}

impl From<&HttpCacheConfig> for CacheOptions {
    fn from(cfg: &HttpCacheConfig) -> Self {
        Self {
            enabled: cfg.enabled,
            max_size_bytes: cfg.max_size_mb.saturating_mul(1024 * 1024),
            default_ttl: Duration::from_secs(cfg.default_ttl_seconds),
        }
    }
}

pub struct CacheManager {
    pub enabled: bool,
    storage: &'static MemCache,
    eviction: &'static LruManager,
    lock: &'static CacheKeyLockImpl,
    defaults: &'static CacheMetaDefaults,
}

impl std::fmt::Debug for CacheManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CacheManager")
            .field("enabled", &self.enabled)
            .finish()
    }
}

impl Clone for CacheManager {
    fn clone(&self) -> Self {
        Self {
            enabled: self.enabled,
            storage: self.storage,
            eviction: self.eviction,
            lock: self.lock,
            defaults: self.defaults,
        }
    }
}

static CACHE_DEFAULT_TTL_SECS: AtomicU64 = AtomicU64::new(60);

fn fresh_duration_for_status(status: http::StatusCode) -> Option<Duration> {
    let secs = CACHE_DEFAULT_TTL_SECS.load(Ordering::Relaxed);
    if secs == 0 {
        return None;
    }
    if status.is_success() || status == http::StatusCode::NOT_FOUND {
        Some(Duration::from_secs(secs))
    } else {
        None
    }
}

impl CacheManager {
    pub fn new(options: CacheOptions) -> Arc<Self> {
        if !options.enabled {
            return Arc::new(Self::disabled());
        }

        CACHE_DEFAULT_TTL_SECS.store(options.default_ttl.as_secs(), Ordering::Relaxed);

        let storage: &'static MemCache = Box::leak(Box::new(MemCache::new()));
        let eviction: &'static LruManager =
            Box::leak(Box::new(LruManager::new(options.max_size_bytes)));
        let lock: &'static CacheLock = Box::leak(CacheLock::new_boxed(Duration::from_secs(10)));
        let lock_dyn: &'static CacheKeyLockImpl = lock;

        let defaults: &'static CacheMetaDefaults = Box::leak(Box::new(CacheMetaDefaults::new(
            fresh_duration_for_status,
            0,
            0,
        )));

        Arc::new(Self {
            enabled: true,
            storage,
            eviction,
            lock: lock_dyn,
            defaults,
        })
    }

    fn disabled() -> Self {
        let storage: &'static MemCache = Box::leak(Box::new(MemCache::new()));
        let eviction: &'static LruManager = Box::leak(Box::new(LruManager::new(1)));
        let lock: &'static CacheLock = Box::leak(CacheLock::new_boxed(Duration::from_secs(10)));
        let lock_dyn: &'static CacheKeyLockImpl = lock;
        let defaults: &'static CacheMetaDefaults =
            Box::leak(Box::new(CacheMetaDefaults::new(|_| None, 0, 0)));

        Self {
            enabled: false,
            storage,
            eviction,
            lock: lock_dyn,
            defaults,
        }
    }

    pub fn create_cache(&self) -> HttpCache {
        let mut cache = HttpCache::new();
        if self.enabled {
            cache.enable(
                self.storage as &'static (dyn Storage + Sync),
                Some(
                    self.eviction as &'static (dyn pingora_cache::eviction::EvictionManager + Sync),
                ),
                None,
                Some(self.lock),
                None,
            );
        }
        cache
    }

    pub fn generate_key(
        &self,
        route_namespace: &str,
        route_name: &str,
        host: &str,
        path: &str,
    ) -> CacheKey {
        let route = format!("{}/{}", route_namespace, route_name);
        let url = format!("{}://{}{}", "https", host, path);
        CacheKey::new(route.as_bytes(), url.as_bytes(), "")
    }

    pub fn is_request_cacheable(req_header: &pingora::http::RequestHeader) -> bool {
        request_cacheable(req_header)
    }

    pub fn is_response_cacheable(
        &self,
        resp: &pingora::http::ResponseHeader,
        cache_control: Option<&CacheControl>,
        has_auth: bool,
    ) -> Option<CacheMeta> {
        match resp_cacheable(cache_control, resp.clone(), has_auth, self.defaults) {
            RespCacheable::Cacheable(meta) => Some(meta),
            RespCacheable::Uncacheable(_) => None,
        }
    }
}
