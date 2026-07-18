use std::collections::BTreeMap;
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::Context;
use base64::Engine as _;
use reqwest::header::{self, HeaderMap, HeaderValue};
use serde::Serialize;

use crate::error::AIError;

/// Langfuse trace ingestion payload.
#[derive(Debug, Serialize)]
struct LangfuseTracePayload {
    #[serde(rename = "traceId")]
    trace_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "userId")]
    user_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "sessionId")]
    session_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    metadata: Option<BTreeMap<String, String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    timestamp: Option<String>,
}

/// Langfuse generation ingestion payload.
#[derive(Debug, Serialize)]
struct LangfuseGenerationPayload {
    #[serde(rename = "traceId")]
    trace_id: String,
    model: String,
    usage: LangfuseUsage,
    #[serde(skip_serializing_if = "Option::is_none")]
    latency: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    input: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    output: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    metadata: Option<BTreeMap<String, String>>,
}

#[derive(Debug, Serialize)]
struct LangfuseUsage {
    input: u64,
    output: u64,
    total: u64,
}

/// Langfuse score ingestion payload.
#[derive(Debug, Serialize)]
struct LangfuseScorePayload {
    #[serde(rename = "traceId")]
    trace_id: String,
    name: String,
    value: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    comment: Option<String>,
}

/// Prompt template fetched from Langfuse Prompt Management.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct PromptTemplate {
    pub name: String,
    pub prompt: String,
    pub version: u32,
    pub config: serde_json::Value,
    pub variables: Vec<String>,
}

/// Payload for creating a Langfuse dataset (v2 API).
#[derive(Debug, Serialize)]
struct DatasetCreatePayload {
    name: String,
}

/// Payload for creating a Langfuse experiment (v2 API).
#[derive(Debug, Serialize)]
struct ExperimentCreatePayload {
    name: String,
}

/// Client for ingesting traces and generations into Langfuse.
pub struct LangfuseClient {
    public_key: String,
    secret_key: String,
    host: String,
    client: reqwest::Client,
    enabled: bool,
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
            usage: LangfuseUsage {
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

        #[derive(Serialize)]
        struct WebhookBody {
            event: String,
            payload: serde_json::Value,
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

/// Format the current UTC time as an ISO 8601 string with millisecond precision.
fn iso8601_now() -> String {
    let since_epoch = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();

    let secs = since_epoch.as_secs();
    let millis = since_epoch.subsec_millis();

    // Days since epoch, then decompose into year/month/day.
    let total_days = secs / 86400;
    let time_secs = secs % 86400;
    let hours = time_secs / 3600;
    let minutes = (time_secs % 3600) / 60;
    let seconds = time_secs % 60;

    let (year, month, day) = days_to_civil(total_days as i64);

    format!("{year:04}-{month:02}-{day:02}T{hours:02}:{minutes:02}:{seconds:02}.{millis:03}Z")
}

/// Convert days since 1970-01-01 to (year, month, day).
fn days_to_civil(mut days: i64) -> (i64, u32, u32) {
    // Algorithm from Howard Hinnant's civil_from_days.
    // Shift epoch from 1970-01-01 to 0000-03-01.
    days += 719468;
    let era = if days >= 0 {
        days / 146097
    } else {
        (days - 146096) / 146097
    };
    let day_of_era = days - era * 146097;
    let year_of_era =
        (day_of_era - day_of_era / 1460 + day_of_era / 36524 - day_of_era / 146096) / 365;
    let year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_ordinal = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_ordinal + 2) / 5 + 1;
    let month = if month_ordinal < 10 {
        month_ordinal + 3
    } else {
        month_ordinal - 9
    };
    let year = if month <= 2 { year + 1 } else { year };

    (year, month as u32, day as u32)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_trace_payload_serialization() {
        let mut metadata = BTreeMap::new();
        metadata.insert("env".to_string(), "staging".to_string());

        let payload = LangfuseTracePayload {
            trace_id: "trace-123".to_string(),
            user_id: Some("user-1".to_string()),
            session_id: Some("sess-abc".to_string()),
            metadata: Some(metadata.clone()),
            timestamp: Some("2026-05-30T12:00:00.000Z".to_string()),
        };

        #[allow(clippy::expect_used)]
        let json = serde_json::to_value(&payload).expect("serialize");

        assert_eq!(json["traceId"], "trace-123");
        assert_eq!(json["userId"], "user-1");
        assert_eq!(json["sessionId"], "sess-abc");
        assert_eq!(json["timestamp"], "2026-05-30T12:00:00.000Z");

        let meta = &json["metadata"];
        assert_eq!(meta["env"], "staging");
    }

    #[test]
    fn test_trace_payload_minimal_serialization() {
        let payload = LangfuseTracePayload {
            trace_id: "trace-minimal".to_string(),
            user_id: None,
            session_id: None,
            metadata: None,
            timestamp: None,
        };

        #[allow(clippy::expect_used)]
        let json = serde_json::to_value(&payload).expect("serialize");
        assert_eq!(json["traceId"], "trace-minimal");
        assert!(json.get("userId").is_none());
        assert!(json.get("sessionId").is_none());
        assert!(json.get("metadata").is_none());
        assert!(json.get("timestamp").is_none());
    }

    #[test]
    fn test_generation_payload_serialization() {
        let mut metadata = BTreeMap::new();
        metadata.insert("source".to_string(), "gateway".to_string());

        let payload = LangfuseGenerationPayload {
            trace_id: "trace-gen-1".to_string(),
            model: "gpt-4".to_string(),
            usage: LangfuseUsage {
                input: 150,
                output: 80,
                total: 230,
            },
            latency: Some(1.234),
            input: Some(serde_json::json!({"prompt": "hello"})),
            output: Some(serde_json::json!({"completion": "world"})),
            metadata: Some(metadata.clone()),
        };

        #[allow(clippy::expect_used)]
        let json = serde_json::to_value(&payload).expect("serialize");

        assert_eq!(json["traceId"], "trace-gen-1");
        assert_eq!(json["model"], "gpt-4");
        assert_eq!(json["usage"]["input"], 150);
        assert_eq!(json["usage"]["output"], 80);
        assert_eq!(json["usage"]["total"], 230);
        assert_eq!(json["latency"], 1.234);
        assert_eq!(json["input"]["prompt"], "hello");
        assert_eq!(json["output"]["completion"], "world");

        let meta = &json["metadata"];
        assert_eq!(meta["source"], "gateway");
    }

    #[test]
    fn test_generation_payload_empty_metadata_omitted() {
        let payload = LangfuseGenerationPayload {
            trace_id: "t".to_string(),
            model: "m".to_string(),
            usage: LangfuseUsage {
                input: 0,
                output: 0,
                total: 0,
            },
            latency: Some(0.0),
            input: None,
            output: None,
            metadata: None,
        };

        #[allow(clippy::expect_used)]
        let json = serde_json::to_value(&payload).expect("serialize");
        assert!(json.get("metadata").is_none());
        assert!(json.get("input").is_none());
        assert!(json.get("output").is_none());
    }

    #[test]
    fn test_noop_client_new_with_empty_key() {
        let client =
            LangfuseClient::new("", "", "").expect("empty public key should disable the client");
        assert!(!client.enabled);
        assert_eq!(client.public_key, "");
        assert_eq!(client.secret_key, "");
        assert_eq!(client.host, "");
    }

    #[test]
    fn test_client_new_with_credentials() {
        let client = LangfuseClient::new("pk-123", "sk-secret", "https://cloud.langfuse.com")
            .expect("valid Langfuse config should construct a client");
        assert!(client.enabled);
        assert_eq!(client.public_key, "pk-123");
        assert_eq!(client.secret_key, "sk-secret");
        assert_eq!(client.host, "https://cloud.langfuse.com");
    }

    #[test]
    fn test_client_new_with_invalid_host() {
        let err = match LangfuseClient::new("pk-123", "sk-secret", "not a url") {
            Ok(_) => panic!("invalid Langfuse host should not construct a client"),
            Err(err) => err,
        };
        assert!(matches!(err, AIError::Observability(_)));
        assert!(err.to_string().contains("invalid Langfuse host URL"));
    }

    #[test]
    fn test_noop_client_method() {
        let client = LangfuseClient::noop_client();
        assert!(!client.enabled);
        assert_eq!(client.public_key, "");
    }

    #[test]
    fn test_iso8601_now_produces_valid_format() {
        let ts = iso8601_now();
        assert_eq!(ts.len(), 24, "ISO 8601 string should be 24 chars");
        assert!(ts.ends_with('Z'), "should end with Z for UTC");
        assert_eq!(&ts[4..5], "-");
        assert_eq!(&ts[7..8], "-");
        assert_eq!(&ts[10..11], "T");
        assert_eq!(&ts[13..14], ":");
        assert_eq!(&ts[16..17], ":");
        assert_eq!(&ts[19..20], ".");
    }

    #[test]
    fn test_days_to_civil_epoch() {
        let (y, m, d) = days_to_civil(0);
        assert_eq!(y, 1970);
        assert_eq!(m, 1);
        assert_eq!(d, 1);
    }

    #[test]
    fn test_days_to_civil_known_date() {
        // 2026-05-30. Days since epoch: compute via known value.
        // 2026-01-01 = day 20454. May 30 is day 150 of 2026 (non-leap).
        // 20454 + 149 = 20603 (since Jan 1 is day 1).
        let (y, m, d) = days_to_civil(20603);
        assert_eq!((y, m, d), (2026, 5, 30));
    }

    #[tokio::test]
    async fn test_noop_ingest_trace_returns_ok() {
        let client = LangfuseClient::noop_client();
        let result = client
            .ingest_trace("trace-1", None, None, &BTreeMap::new())
            .await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_noop_ingest_generation_returns_ok() {
        let client = LangfuseClient::noop_client();
        let result = client
            .ingest_generation(
                "trace-1",
                "gpt-4",
                100,
                50,
                500,
                &serde_json::json!({"prompt": "hi"}),
                &serde_json::json!({"completion": "hello"}),
                &BTreeMap::new(),
            )
            .await;
        assert!(result.is_ok());
    }

    #[test]
    fn test_score_payload_shape() {
        let payload = LangfuseScorePayload {
            trace_id: "trace-abc".to_string(),
            name: "accuracy".to_string(),
            value: 0.95,
            comment: Some("high quality".to_string()),
        };

        #[allow(clippy::expect_used)]
        let json = serde_json::to_value(&payload).expect("serialize");
        assert_eq!(json["traceId"], "trace-abc");
        assert_eq!(json["name"], "accuracy");
        assert_eq!(json["value"], 0.95);
        assert_eq!(json["comment"], "high quality");
    }

    #[test]
    fn test_score_payload_no_comment() {
        let payload = LangfuseScorePayload {
            trace_id: "trace-xyz".to_string(),
            name: "latency".to_string(),
            value: 42.0,
            comment: None,
        };

        #[allow(clippy::expect_used)]
        let json = serde_json::to_value(&payload).expect("serialize");
        assert_eq!(json["name"], "latency");
        assert!(json.get("comment").is_none());
    }

    #[test]
    fn test_prompt_template_deserialization() {
        let raw = serde_json::json!({
            "name": "qa-bot",
            "prompt": "You are a helpful assistant. Context: {{context}}",
            "version": 3,
            "config": {"temperature": 0.7},
            "variables": ["context"]
        });

        #[allow(clippy::expect_used)]
        let tmpl: PromptTemplate = serde_json::from_value(raw).expect("deserialize");
        assert_eq!(tmpl.name, "qa-bot");
        assert_eq!(tmpl.version, 3);
        assert_eq!(tmpl.variables, vec!["context"]);
        assert_eq!(tmpl.config["temperature"], 0.7);
        assert!(tmpl.prompt.contains("{{context}}"));
    }

    #[tokio::test]
    async fn test_noop_ingest_score() {
        let client = LangfuseClient::noop_client();
        let result = client
            .ingest_score("trace-1", "accuracy", 0.9, Some("good"))
            .await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_noop_get_prompt() {
        let client = LangfuseClient::noop_client();
        let result = client.get_prompt("qa-bot", None).await;
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("Langfuse client not enabled")
        );
    }

    #[test]
    fn test_dataset_create_payload_serialization() {
        let payload = DatasetCreatePayload {
            name: "eval-dataset".to_string(),
        };
        #[allow(clippy::expect_used)]
        let json = serde_json::to_value(&payload).expect("serialize");
        assert_eq!(json["name"], "eval-dataset");
    }

    #[test]
    fn test_experiment_create_payload_serialization() {
        let payload = ExperimentCreatePayload {
            name: "exp-1".to_string(),
        };
        #[allow(clippy::expect_used)]
        let json = serde_json::to_value(&payload).expect("serialize");
        assert_eq!(json["name"], "exp-1");
    }

    #[tokio::test]
    async fn test_noop_create_dataset_returns_error() {
        let client = LangfuseClient::noop_client();
        let result = client.create_dataset("my-dataset").await;
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("Langfuse client not enabled")
        );
    }

    #[tokio::test]
    async fn test_noop_create_experiment_returns_error() {
        let client = LangfuseClient::noop_client();
        let result = client
            .create_experiment("my-experiment", "dataset-123")
            .await;
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("Langfuse client not enabled")
        );
    }

    #[tokio::test]
    async fn test_noop_trigger_webhook_returns_error() {
        let client = LangfuseClient::noop_client();
        let result = client
            .trigger_webhook(
                "https://hooks.example.com/ai-gateway",
                "generation.completed",
                &serde_json::json!({"trace_id": "abc-123"}),
            )
            .await;
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("Langfuse client not enabled")
        );
    }
}
