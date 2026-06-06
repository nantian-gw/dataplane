/// Errors for AI Gateway operations.
#[derive(Debug, thiserror::Error)]
pub enum AIError {
    #[error("format detection failed: {0}")]
    FormatDetection(String),

    #[error("format parse error ({format}): {message}")]
    FormatParse { format: String, message: String },

    #[error("format serialize error ({format}): {message}")]
    FormatSerialize { format: String, message: String },

    #[error("unsupported format: {0}")]
    UnsupportedFormat(String),

    #[error("adapter not found: {0}")]
    AdapterNotFound(String),

    #[error("token counter error: {0}")]
    TokenCounter(String),

    #[error("observability error: {0}")]
    Observability(String),

    #[error("backend error: status={status}, body={body}")]
    Backend { status: u16, body: String },

    #[error("rate limit exceeded: retry_after_secs={retry_after_secs}")]
    RateLimitExceeded { retry_after_secs: u64 },

    #[error(transparent)]
    Internal(#[from] anyhow::Error),

    #[error("prompt guard blocked: {reason}, matched: {matched}")]
    PromptGuardBlocked { reason: String, matched: String },

    #[error("content safety blocked: {category}, matched: {matched}")]
    ContentSafetyBlocked { category: String, matched: String },

    #[error("cache hit")]
    CacheHit {
        response: crate::format::ir::AIResponse,
    },

    #[error("all fallbacks exhausted for model {model}: {reason}")]
    FallbackExhausted { model: String, reason: String },

    #[error("tenant access denied: {0}")]
    TenantDenied(String),
}
