use std::sync::Arc;

use ntgw_wasm::sandbox::AISandbox;

use crate::error::AIError;
use crate::format::ir::{AIStreamChunk, AIUsage};

/// Accumulates token usage from AI responses and streaming chunks.
///
/// `TokenCounter` can be used:
/// - With `record_response` for non-streaming usage from an `AIUsage`.
/// - With `record_stream_chunk` to accumulate usage from SSE chunks.
/// - With `from_sse_body` to parse a raw SSE body and extract the final
///   accumulated usage together with the concatenated content deltas.
/// - With `count_tokens` to count tokens in arbitrary text, using a sandbox
///   Wasm tokenizer if available, otherwise falling back to approximation.
#[derive(Debug, Clone, Default)]
pub struct TokenCounter {
    pub prompt_tokens: u64,
    pub completion_tokens: u64,
    pub total_tokens: u64,
    ai_sandbox: Option<Arc<AISandbox>>,
}

impl TokenCounter {
    /// Creates a new `TokenCounter` with all accumulators set to zero.
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates a new `TokenCounter` backed by the given sandbox for accurate token counting.
    pub fn with_sandbox(sandbox: Arc<AISandbox>) -> Self {
        Self {
            ai_sandbox: Some(sandbox),
            ..Default::default()
        }
    }

    /// Count tokens in `text` using the sandbox tokenizer if available,
    /// falling back to an approximate word-count heuristic.
    pub fn count_tokens(&self, text: &str) -> u64 {
        if let Some(ref sandbox) = self.ai_sandbox {
            sandbox
                .tokenize("tokenizer-simple", text)
                .map(|ids| ids.len() as u64)
                .unwrap_or_else(|_| approximate_token_count(text))
        } else {
            approximate_token_count(text)
        }
    }

    /// Records usage from a non-streaming response, adding the fields to the
    /// accumulators.
    pub fn record_response(&mut self, usage: &AIUsage) {
        self.prompt_tokens += usage.prompt_tokens;
        self.completion_tokens += usage.completion_tokens;
        self.total_tokens += usage.total_tokens;
    }

    /// Records usage from a single SSE streaming chunk.
    ///
    /// If the chunk carries a `usage` field (typically only on the last chunk),
    /// its values are added to the accumulators.
    pub fn record_stream_chunk(&mut self, chunk: &AIStreamChunk) {
        if let Some(ref usage) = chunk.usage {
            self.prompt_tokens += usage.prompt_tokens;
            self.completion_tokens += usage.completion_tokens;
            self.total_tokens += usage.total_tokens;
        }
    }

    /// Returns the current accumulated usage as an `AIUsage` struct.
    pub fn accumulated_usage(&self) -> AIUsage {
        AIUsage {
            prompt_tokens: self.prompt_tokens,
            completion_tokens: self.completion_tokens,
            total_tokens: self.total_tokens,
        }
    }

    /// Parses a raw SSE stream body and returns the accumulated usage together
    /// with the last concatenated delta content.
    ///
    /// SSE events are delimited by `\n\n`. Each event line is expected to start
    /// with `data: `. Lines that do not start with `data: ` are silently skipped.
    /// `data: [DONE]` lines are ignored. Every other `data:` payload is parsed
    /// as a JSON `AIStreamChunk`.
    ///
    /// # Errors
    ///
    /// Returns `AIError::Internal` if any chunk fails to deserialise.
    pub fn from_sse_body(body: &[u8]) -> Result<(AIUsage, String), AIError> {
        let text = std::str::from_utf8(body)
            .map_err(|e| AIError::Internal(anyhow::anyhow!("SSE body is not valid UTF-8: {e}")))?;

        let mut counter = TokenCounter::new();
        let mut content_parts: Vec<String> = Vec::new();

        // Events are separated by blank lines (\n\n)
        for event in text.split("\n\n") {
            let event = event.trim();
            if event.is_empty() {
                continue;
            }
            for line in event.lines() {
                let line = line.trim();
                if let Some(data) = line.strip_prefix("data: ") {
                    // Skip the stream termination marker
                    if data == "[DONE]" {
                        break;
                    }

                    let chunk: AIStreamChunk = serde_json::from_str(data).map_err(|e| {
                        AIError::Internal(anyhow::anyhow!("Failed to parse SSE chunk: {e}"))
                    })?;

                    counter.record_stream_chunk(&chunk);

                    // Concatenate delta content from all choices
                    for choice in &chunk.choices {
                        if let Some(ref content) = choice.delta.content {
                            content_parts.push(content.clone());
                        }
                    }
                }
            }
        }

        let last_content = content_parts.concat();
        Ok((counter.accumulated_usage(), last_content))
    }
}

/// Fallback approximate token count: word count × 1.3 for subword tokens.
fn approximate_token_count(text: &str) -> u64 {
    let word_count = text.split_whitespace().count() as f64;
    (word_count * 1.3) as u64
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::format::ir::AIUsage;

    #[test]
    fn test_new() {
        let tc = TokenCounter::new();
        assert_eq!(tc.prompt_tokens, 0);
        assert_eq!(tc.completion_tokens, 0);
        assert_eq!(tc.total_tokens, 0);
    }

    #[test]
    fn test_record_response() {
        let mut tc = TokenCounter::new();
        let usage = AIUsage {
            prompt_tokens: 10,
            completion_tokens: 5,
            total_tokens: 15,
        };
        tc.record_response(&usage);
        assert_eq!(tc.prompt_tokens, 10);
        assert_eq!(tc.completion_tokens, 5);
        assert_eq!(tc.total_tokens, 15);
    }

    #[test]
    fn test_record_response_accumulates() {
        let mut tc = TokenCounter::new();
        tc.record_response(&AIUsage {
            prompt_tokens: 10,
            completion_tokens: 5,
            total_tokens: 15,
        });
        tc.record_response(&AIUsage {
            prompt_tokens: 20,
            completion_tokens: 10,
            total_tokens: 30,
        });
        assert_eq!(tc.prompt_tokens, 30);
        assert_eq!(tc.completion_tokens, 15);
        assert_eq!(tc.total_tokens, 45);
    }

    #[test]
    fn test_record_stream_chunk_with_usage() {
        let mut tc = TokenCounter::new();
        let chunk = AIStreamChunk {
            id: "1".into(),
            model: "gpt-4".into(),
            choices: vec![],
            usage: Some(AIUsage {
                prompt_tokens: 100,
                completion_tokens: 50,
                total_tokens: 150,
            }),
            created: None,
        };
        tc.record_stream_chunk(&chunk);
        assert_eq!(tc.prompt_tokens, 100);
        assert_eq!(tc.completion_tokens, 50);
        assert_eq!(tc.total_tokens, 150);
    }

    #[test]
    fn test_record_stream_chunk_without_usage() {
        let mut tc = TokenCounter::new();
        let chunk = AIStreamChunk {
            id: "1".into(),
            model: "gpt-4".into(),
            choices: vec![],
            usage: None,
            created: None,
        };
        tc.record_stream_chunk(&chunk);
        assert_eq!(tc.prompt_tokens, 0);
        assert_eq!(tc.completion_tokens, 0);
        assert_eq!(tc.total_tokens, 0);
    }

    #[test]
    fn test_accumulated_usage() {
        let mut tc = TokenCounter::new();
        tc.record_response(&AIUsage {
            prompt_tokens: 10,
            completion_tokens: 20,
            total_tokens: 30,
        });
        let usage = tc.accumulated_usage();
        assert_eq!(usage.prompt_tokens, 10);
        assert_eq!(usage.completion_tokens, 20);
        assert_eq!(usage.total_tokens, 30);
    }

    #[test]
    fn test_from_sse_body_basic() {
        let body = b"data: {\"id\":\"1\",\"model\":\"gpt-4\",\"choices\":[{\"index\":0,\"delta\":{\"content\":\"Hello\"}}],\"usage\":null}\n\ndata: {\"id\":\"2\",\"model\":\"gpt-4\",\"choices\":[{\"index\":0,\"delta\":{\"content\":\" world\"}}],\"usage\":null}\n\ndata: {\"id\":\"3\",\"model\":\"gpt-4\",\"choices\":[{\"index\":0,\"delta\":{\"content\":\"\"},\"finish_reason\":\"stop\"}],\"usage\":{\"prompt_tokens\":10,\"completion_tokens\":5,\"total_tokens\":15}}\n\ndata: [DONE]\n\n";

        #[allow(clippy::unwrap_used)]
        let (usage, content) = TokenCounter::from_sse_body(body).unwrap();
        assert_eq!(usage.prompt_tokens, 10);
        assert_eq!(usage.completion_tokens, 5);
        assert_eq!(usage.total_tokens, 15);
        assert_eq!(content, "Hello world");
    }

    #[test]
    fn test_from_sse_body_no_usage() {
        let body = b"data: {\"id\":\"1\",\"model\":\"gpt-4\",\"choices\":[{\"index\":0,\"delta\":{\"content\":\"Hi\"}}],\"usage\":null}\n\ndata: [DONE]\n\n";

        #[allow(clippy::unwrap_used)]
        let (usage, content) = TokenCounter::from_sse_body(body).unwrap();
        assert_eq!(usage.prompt_tokens, 0);
        assert_eq!(usage.completion_tokens, 0);
        assert_eq!(usage.total_tokens, 0);
        assert_eq!(content, "Hi");
    }

    #[test]
    fn test_from_sse_body_empty() {
        let body = b"";
        #[allow(clippy::unwrap_used)]
        let (usage, content) = TokenCounter::from_sse_body(body).unwrap();
        assert_eq!(usage.prompt_tokens, 0);
        assert_eq!(usage.completion_tokens, 0);
        assert_eq!(usage.total_tokens, 0);
        assert_eq!(content, "");
    }

    #[test]
    fn test_default() {
        let tc = TokenCounter::default();
        assert_eq!(tc.prompt_tokens, 0);
        assert_eq!(tc.completion_tokens, 0);
        assert_eq!(tc.total_tokens, 0);
    }
}
