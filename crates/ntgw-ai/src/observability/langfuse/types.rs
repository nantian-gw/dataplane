use std::collections::BTreeMap;

use serde::Serialize;

/// Langfuse trace ingestion payload.
#[derive(Debug, Serialize)]
pub(crate) struct LangfuseTracePayload {
    #[serde(rename = "traceId")]
    pub(crate) trace_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "userId")]
    pub(crate) user_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "sessionId")]
    pub(crate) session_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) metadata: Option<BTreeMap<String, String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) timestamp: Option<String>,
}

/// Langfuse generation ingestion payload.
#[derive(Debug, Serialize)]
pub(crate) struct LangfuseGenerationPayload {
    #[serde(rename = "traceId")]
    pub(crate) trace_id: String,
    pub(crate) model: String,
    pub(crate) usage: LangfuseUsage,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) latency: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) input: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) output: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) metadata: Option<BTreeMap<String, String>>,
}

#[derive(Debug, Serialize)]
pub(crate) struct LangfuseUsage {
    pub(crate) input: u64,
    pub(crate) output: u64,
    pub(crate) total: u64,
}

/// Langfuse score ingestion payload.
#[derive(Debug, Serialize)]
pub(crate) struct LangfuseScorePayload {
    #[serde(rename = "traceId")]
    pub(crate) trace_id: String,
    pub(crate) name: String,
    pub(crate) value: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) comment: Option<String>,
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
pub(crate) struct DatasetCreatePayload {
    pub(crate) name: String,
}

/// Payload for creating a Langfuse experiment (v2 API).
#[derive(Debug, Serialize)]
pub(crate) struct ExperimentCreatePayload {
    pub(crate) name: String,
}

#[derive(Serialize)]
pub(crate) struct WebhookBody {
    pub(crate) event: String,
    pub(crate) payload: serde_json::Value,
}
