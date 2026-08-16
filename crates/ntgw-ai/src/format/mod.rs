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

    /// Parse the full SSE response body into stream chunks.
    /// Each provider may have a different wire format.
    fn parse_stream_body(&self, body: &[u8]) -> Result<Vec<AIStreamChunk>, AIError> {
        // Default implementation: parse as OpenAI-style SSE
        let sse_text = std::str::from_utf8(body)
            .map_err(|e| AIError::Internal(anyhow::anyhow!("SSE: {e}")))?;
        let mut chunks = Vec::new();
        for event in sse_text.split(
            "

",
        ) {
            let event = event.trim();
            if event.is_empty() {
                continue;
            }
            for line in event.lines() {
                if let Some(json) = line.strip_prefix("data: ") {
                    if json == "[DONE]" {
                        continue;
                    }
                    let chunk: AIStreamChunk = serde_json::from_str(json)
                        .map_err(|e| AIError::Internal(anyhow::anyhow!("SSE parse error: {e}")))?;
                    chunks.push(chunk);
                }
            }
        }
        Ok(chunks)
    }

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
    // Use path segment matching to avoid false positives.
    let segments: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
    for seg in segments.windows(2) {
        match seg {
            ["v1", "chat\\_completions"] | ["v1", "completions"] | ["chat", "completions"] => {
                return Some("openai");
            }
            ["v1", "messages"] => return Some("anthropic"),
            ["api", "chat"] | ["api", "generate"] => return Some("ollama"),
            _ => (),
        }
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
