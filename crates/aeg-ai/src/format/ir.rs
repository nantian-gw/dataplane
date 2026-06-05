use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// Internal representation of an AI chat completion request.
/// All format adapters produce this IR; all filters consume this IR.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AIRequest {
    pub messages: Vec<AIMessage>,
    pub model: String,
    #[serde(default)]
    pub temperature: Option<f32>,
    #[serde(default)]
    pub max_tokens: Option<u32>,
    #[serde(default)]
    pub top_p: Option<f32>,
    #[serde(default)]
    pub stop: Vec<String>,
    #[serde(default)]
    pub stream: bool,
    #[serde(default)]
    pub user: Option<String>,
    #[serde(default)]
    pub extra: BTreeMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AIMessage {
    pub role: AIRole,
    pub content: AIContent,
    pub name: Option<String>,
    #[serde(default)]
    pub tool_calls: Vec<AIToolCall>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum AIRole {
    System,
    User,
    Assistant,
    Tool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum AIContent {
    Text(String),
    MultiPart(Vec<AIContentPart>),
    #[serde(deserialize_with = "deserialize_null_as_none")]
    None,
}

fn deserialize_null_as_none<'de, D>(deserializer: D) -> Result<(), D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::de;
    struct NullVisitor;
    impl de::Visitor<'_> for NullVisitor {
        type Value = ();
        fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
            f.write_str("null")
        }
        fn visit_unit<E: de::Error>(self) -> Result<Self::Value, E> {
            Ok(())
        }
    }
    deserializer.deserialize_unit(NullVisitor)?;
    Ok(())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AIContentPart {
    #[serde(rename = "type")]
    pub content_type: String,
    pub text: Option<String>,
    pub image_url: Option<AIImageUrl>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AIImageUrl {
    pub url: String,
    pub detail: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AIToolCall {
    pub id: String,
    #[serde(rename = "type")]
    pub call_type: String,
    pub function: AIToolCallFunction,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AIToolCallFunction {
    pub name: String,
    pub arguments: String,
}

/// Internal representation of an AI chat completion response (non-streaming).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AIResponse {
    pub id: String,
    pub model: String,
    pub choices: Vec<AIChoice>,
    #[serde(default)]
    pub usage: Option<AIUsage>,
    #[serde(default)]
    pub created: Option<u64>,
    #[serde(default)]
    pub extra: BTreeMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AIChoice {
    pub index: u32,
    pub message: AIMessage,
    #[serde(default)]
    pub finish_reason: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AIUsage {
    pub prompt_tokens: u64,
    pub completion_tokens: u64,
    pub total_tokens: u64,
}

/// Streaming chunk (SSE event data).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AIStreamChunk {
    pub id: String,
    pub model: String,
    pub choices: Vec<AIStreamChoice>,
    #[serde(default)]
    pub usage: Option<AIUsage>,
    #[serde(default)]
    pub created: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AIStreamChoice {
    pub index: u32,
    pub delta: AIStreamDelta,
    #[serde(default)]
    pub finish_reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AIStreamDelta {
    pub role: Option<AIRole>,
    pub content: Option<String>,
    #[serde(default)]
    pub tool_calls: Vec<AIToolCall>,
}
