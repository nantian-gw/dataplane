use std::collections::BTreeMap;

use anyhow::Context;
use base64::Engine as _;
use reqwest::header::{self, HeaderMap, HeaderValue};

use crate::error::AIError;

use super::helpers::iso8601_now;
use super::types::{
    DatasetCreatePayload, ExperimentCreatePayload, LangfuseGenerationPayload, LangfuseScorePayload,
    LangfuseTracePayload, PromptTemplate, WebhookBody,
};

/// Client for ingesting traces and generations into Langfuse.
pub struct LangfuseClient {
    pub(crate) public_key: String,
    pub(crate) secret_key: String,
    pub(crate) host: String,
    pub(crate) client: reqwest::Client,
    pub(crate) enabled: bool,
}

impl LangfuseClient {
    /// Create a new LangfuseClient with the given credentials and host.
    ///
    /// If `public_key` is empty, the client operates in noop mode: all
    /// ingest methods return `Ok(())` without making HTTP calls.
    pub fn new(public_key: &str, secret_key: &str, host: &str) -> Result<Self, AIError> {
        let enabled = !public_key.is_empty();

        if !enabled {
            return Ok(Self {
                public_key: public_key.to_string(),
                secret_key: secret_key.to_string(),
                host: host.to_string(),
                client: reqwest::Client::new(),
                enabled: false,
            });
        }

        reqwest::Url::parse(host).map_err(|e| {
            AIError::Observability(format!("invalid Langfuse host URL `{host}`: {e}"))
        })?;

        let basic =
            base64::engine::general_purpose::STANDARD.encode(format!("{public_key}:{secret_key}"));
        let auth_value = HeaderValue::from_str(&format!("Basic {basic}")).map_err(|e| {
            AIError::Observability(format!(
                "failed to build Langfuse authorization header: {e}"
            ))
        })?;
        let mut headers = HeaderMap::new();
        headers.insert(header::AUTHORIZATION, auth_value);
        headers.insert(
            header::CONTENT_TYPE,
            HeaderValue::from_static("application/json"),
        );

        let client = reqwest::Client::builder()
            .default_headers(headers)
            .timeout(std::time::Duration::from_secs(10))
            .connect_timeout(std::time::Duration::from_secs(5))
            .build()
            .map_err(|e| {
                AIError::Observability(format!("failed to build Langfuse HTTP client: {e}"))
            })?;

        Ok(Self {
            public_key: public_key.to_string(),
            secret_key: secret_key.to_string(),
            host: host.to_string(),
            client,
            enabled: true,
        })
    }

    /// Create a noop client that silently discards all ingest calls.
    pub fn noop_client() -> Self {
        Self {
            public_key: String::new(),
            secret_key: String::new(),
            host: String::new(),
            client: reqwest::Client::new(),
            enabled: false,
        }
    }

    /// Returns whether this client is enabled (will send HTTP requests).
    pub fn enabled(&self) -> bool {
        self.enabled
    }

    /// Returns the public key used for authentication.
    pub fn public_key(&self) -> &str {
        &self.public_key
    }

    /// Returns the secret key used for authentication.
    pub fn secret_key(&self) -> &str {
        &self.secret_key
    }

    /// Returns the Langfuse host URL.
    pub fn host(&self) -> &str {
        &self.host
    }

    /// Ingest a trace into Langfuse.
    pub async fn ingest_trace(
        &self,
        trace_id: &str,
        user_id: Option<&str>,
        session_id: Option<&str>,
        metadata: &BTreeMap<String, String>,
    ) -> Result<(), anyhow::Error> {
        if !self.enabled {
            return Ok(());
        }

        let payload = LangfuseTracePayload {
            trace_id: trace_id.to_string(),
            user_id: user_id.map(|s| s.to_string()),
            session_id: session_id.map(|s| s.to_string()),
            metadata: if metadata.is_empty() {
                None
            } else {
                Some(metadata.clone())
            },
            timestamp: Some(iso8601_now()),
        };

        let url = format!("{}/api/public/traces", self.host);
        let resp = self
            .client
            .post(&url)
            .json(&payload)
            .send()
            .await
            .context("failed to send trace ingestion request")?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            anyhow::bail!("trace ingestion failed with status {status}: {body}");
        }

        Ok(())
    }

    /// Ingest a generation into Langfuse.
    #[allow(clippy::too_many_arguments)]
    pub async fn ingest_generation(
        &self,
        trace_id: &str,
        model: &str,
        input_tokens: u64,
        output_tokens: u64,
        latency_ms: u64,
        input: &serde_json::Value,
        output: &serde_json::Value,
        metadata: &BTreeMap<String, String>,
    ) -> Result<(), anyhow::Error> {
        if !self.enabled {
            return Ok(());
        }

        let payload = LangfuseGenerationPayload {
            trace_id: trace_id.to_string(),
            model: model.to_string(),
            usage: super::types::LangfuseUsage {
                input: input_tokens,
                output: output_tokens,
                total: input_tokens + output_tokens,
            },
            latency: Some(latency_ms as f64 / 1000.0),
            input: Some(input.clone()),
            output: Some(output.clone()),
            metadata: if metadata.is_empty() {
                None
            } else {
                Some(metadata.clone())
            },
        };

        let url = format!("{}/api/public/generations", self.host);
        let resp = self
            .client
            .post(&url)
            .json(&payload)
            .send()
            .await
            .context("failed to send generation ingestion request")?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            anyhow::bail!("generation ingestion failed with status {status}: {body}");
        }

        Ok(())
    }

    /// Ingest a score (evaluation metric) for a trace.
    pub async fn ingest_score(
        &self,
        trace_id: &str,
        name: &str,
        value: f64,
        comment: Option<&str>,
    ) -> Result<(), anyhow::Error> {
        if !self.enabled {
            return Ok(());
        }

        let payload = LangfuseScorePayload {
            trace_id: trace_id.to_string(),
            name: name.to_string(),
            value,
            comment: comment.map(|s| s.to_string()),
        };

        let url = format!("{}/api/public/scores", self.host);
        let resp = self
            .client
            .post(&url)
            .json(&payload)
            .send()
            .await
            .context("failed to send score ingestion request")?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            anyhow::bail!("score ingestion failed with status {status}: {body}");
        }

        Ok(())
    }

    /// Fetch a prompt template from Langfuse Prompt Management.
    pub async fn get_prompt(
        &self,
        name: &str,
        version: Option<u32>,
    ) -> Result<PromptTemplate, anyhow::Error> {
        if !self.enabled {
            return Err(anyhow::anyhow!("Langfuse client not enabled"));
        }

        let mut url = format!("{}/api/public/v2/prompts/{}", self.host, name);
        if let Some(v) = version {
            url.push_str(&format!("?version={v}"));
        }

        let resp = self
            .client
            .get(&url)
            .send()
            .await
            .context("failed to fetch prompt")?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            anyhow::bail!("prompt fetch failed with status {status}: {body}");
        }

        let template: PromptTemplate = resp
            .json()
            .await
            .context("failed to deserialize prompt template")?;
        Ok(template)
    }

    /// Create a dataset via the Langfuse v2 API.
    ///
    /// Returns an error if the client is disabled.
    pub async fn create_dataset(&self, name: &str) -> Result<(), anyhow::Error> {
        if !self.enabled {
            return Err(anyhow::anyhow!("Langfuse client not enabled"));
        }

        let payload = DatasetCreatePayload {
            name: name.to_string(),
        };
        let url = format!("{}/api/public/v2/datasets", self.host);
        let resp = self
            .client
            .post(&url)
            .json(&payload)
            .send()
            .await
            .context("failed to create dataset")?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            anyhow::bail!("dataset creation failed with status {status}: {body}");
        }
        Ok(())
    }

    /// Create an experiment for a dataset via the Langfuse v2 API.
    ///
    /// Returns an error if the client is disabled.
    pub async fn create_experiment(
        &self,
        name: &str,
        dataset_id: &str,
    ) -> Result<(), anyhow::Error> {
        if !self.enabled {
            return Err(anyhow::anyhow!("Langfuse client not enabled"));
        }

        let payload = ExperimentCreatePayload {
            name: name.to_string(),
        };
        let url = format!(
            "{}/api/public/v2/datasets/{}/experiments",
            self.host, dataset_id
        );
        let resp = self
            .client
            .post(&url)
            .json(&payload)
            .send()
            .await
            .context("failed to create experiment")?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            anyhow::bail!("experiment creation failed with status {status}: {body}");
        }
        Ok(())
    }

    /// Send a webhook event to a configurable URL.
    ///
    /// Returns an error if the client is disabled.
    pub async fn trigger_webhook(
        &self,
        url: &str,
        event: &str,
        payload: &serde_json::Value,
    ) -> Result<(), anyhow::Error> {
        if !self.enabled {
            return Err(anyhow::anyhow!("Langfuse client not enabled"));
        }

        let body = WebhookBody {
            event: event.to_string(),
            payload: payload.clone(),
        };
        let resp = self
            .client
            .post(url)
            .json(&body)
            .send()
            .await
            .with_context(|| format!("failed to send webhook to {url}"))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body_text = resp.text().await.unwrap_or_default();
            anyhow::bail!("webhook to {url} failed with status {status}: {body_text}");
        }
        Ok(())
    }
}
