use super::*;
use std::sync::Arc;

pub(super) fn build_langfuse_client() -> Option<Arc<ntgw_ai::observability::langfuse::LangfuseClient>> {
    use ntgw_ai::observability::langfuse::LangfuseClient;

    let public_key = std::env::var("LANGFUSE_PUBLIC_KEY").unwrap_or_default();
    let secret_key = std::env::var("LANGFUSE_SECRET_KEY").unwrap_or_default();
    let host = std::env::var("LANGFUSE_HOST").unwrap_or_default();

    if public_key.is_empty() || secret_key.is_empty() || host.is_empty() {
        return None;
    }

    match LangfuseClient::new(&public_key, &secret_key, &host) {
        Ok(client) => {
            tracing::info!(
                target: "ai_gateway",
                host = %host,
                "langfuse observability enabled"
            );
            Some(Arc::new(client))
        }
        Err(e) => {
            tracing::warn!(
                target: "ai_gateway",
                error = %e,
                "failed to create langfuse client, langfuse disabled"
            );
            None
        }
    }
}
