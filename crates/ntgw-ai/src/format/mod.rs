pub mod anthropic;
pub mod ir;
pub mod ollama;
pub mod openai;

pub use ir::*;

use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;

use crate::error::AIError;

#[async_trait]
pub trait FormatAdapter: Send + Sync {
    fn name(&self) -> &'static str;

    fn parse_request(&self, body: &[u8]) -> Result<AIRequest, AIError>;

    fn parse_response(&self, body: &[u8]) -> Result<AIResponse, AIError>;

    fn serialize_response(&self, response: &AIResponse) -> Result<Vec<u8>, AIError>;

    fn serialize_stream_chunk(&self, chunk: &AIStreamChunk) -> Result<String, AIError>;

    fn error_response(&self, status: u16, message: &str) -> Result<Vec<u8>, AIError>;
}

pub struct AdapterRegistry {
    adapters: HashMap<String, Arc<dyn FormatAdapter>>,
}

impl AdapterRegistry {
    pub fn new() -> Self {
        Self {
            adapters: HashMap::new(),
        }
    }

    pub fn register(&mut self, name: impl Into<String>, adapter: Arc<dyn FormatAdapter>) {
        self.adapters.insert(name.into(), adapter);
    }

    #[must_use]
    pub fn get(&self, name: &str) -> Option<&dyn FormatAdapter> {
        self.adapters.get(name).map(|a| a.as_ref())
    }

    pub fn names(&self) -> Vec<&str> {
        self.adapters.keys().map(|k| k.as_str()).collect()
    }
}

impl Default for AdapterRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[must_use]
pub fn detect_format(path: &str) -> Option<&'static str> {
    if path.contains("/v1/chat/completions")
        || path.contains("/v1/completions")
        || path.contains("/chat/completions")
    {
        return Some("openai");
    }
    if path.contains("/v1/messages") {
        return Some("anthropic");
    }
    if path.contains("/api/chat") || path.contains("/api/generate") {
        return Some("ollama");
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_openai() {
        assert_eq!(detect_format("/v1/chat/completions"), Some("openai"));
        assert_eq!(detect_format("/v1/completions"), Some("openai"));
        assert_eq!(
            detect_format("/openai/deployments/gpt4o/chat/completions"),
            Some("openai")
        );
    }

    #[test]
    fn test_detect_anthropic() {
        assert_eq!(detect_format("/v1/messages"), Some("anthropic"));
        assert_eq!(detect_format("/v1/messages/msg_123"), Some("anthropic"));
    }

    #[test]
    fn test_detect_ollama() {
        assert_eq!(detect_format("/api/chat"), Some("ollama"));
        assert_eq!(detect_format("/api/generate"), Some("ollama"));
    }

    #[test]
    fn test_detect_unknown() {
        assert_eq!(detect_format("/healthz"), None);
        assert_eq!(detect_format("/v1/models"), None);
        assert_eq!(detect_format("/"), None);
    }

    #[test]
    fn test_registry_register_and_get() {
        let registry = AdapterRegistry::new();
        assert!(registry.get("openai").is_none());

        assert_eq!(registry.names().len(), 0);
    }
}
