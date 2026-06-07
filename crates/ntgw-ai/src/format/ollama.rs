use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::error::AIError;
use crate::format::ir::*;
use crate::format::FormatAdapter;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct OllamaChatRequest {
    model: String,
    messages: Vec<OllamaMessage>,
    #[serde(default)]
    stream: Option<bool>,
    #[serde(default)]
    options: Option<OllamaOptions>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct OllamaMessage {
    role: String,
    content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct OllamaOptions {
    #[serde(default)]
    temperature: Option<f32>,
    #[serde(default)]
    top_p: Option<f32>,
    #[serde(default)]
    num_predict: Option<u32>,
    #[serde(default)]
    stop: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct OllamaChatResponse {
    model: String,
    created_at: String,
    message: OllamaMessage,
    done: bool,
    #[serde(default)]
    total_duration: Option<u64>,
    #[serde(default)]
    eval_count: Option<u64>,
    #[serde(default)]
    prompt_eval_count: Option<u64>,
}

impl From<OllamaMessage> for AIMessage {
    fn from(msg: OllamaMessage) -> Self {
        let role = match msg.role.as_str() {
            "system" => AIRole::System,
            "user" => AIRole::User,
            "assistant" => AIRole::Assistant,
            _ => AIRole::User,
        };
        AIMessage {
            role,
            content: AIContent::Text(msg.content),
            name: None,
            tool_calls: vec![],
            tool_call_id: None,
        }
    }
}

impl From<OllamaChatRequest> for AIRequest {
    fn from(req: OllamaChatRequest) -> Self {
        let options = req.options.unwrap_or_default();
        AIRequest {
            model: req.model,
            messages: req.messages.into_iter().map(AIMessage::from).collect(),
            temperature: options.temperature,
            max_tokens: options.num_predict,
            top_p: options.top_p,
            stop: options.stop.unwrap_or_default(),
            stream: req.stream.unwrap_or(false),
            user: None,
            extra: Default::default(),
        }
    }
}

impl From<&AIResponse> for OllamaChatResponse {
    fn from(resp: &AIResponse) -> Self {
        let first = resp.choices.first();
        let content = first
            .map(|c| match &c.message.content {
                AIContent::Text(text) => text.clone(),
                _ => String::new(),
            })
            .unwrap_or_default();
        let role = first
            .map(|c| match c.message.role {
                AIRole::Assistant => "assistant",
                _ => "assistant",
            })
            .unwrap_or("assistant");

        OllamaChatResponse {
            model: resp.model.clone(),
            created_at: String::new(),
            message: OllamaMessage {
                role: role.into(),
                content,
            },
            done: true,
            total_duration: None,
            eval_count: resp.usage.as_ref().map(|u| u.completion_tokens),
            prompt_eval_count: resp.usage.as_ref().map(|u| u.prompt_tokens),
        }
    }
}

impl From<OllamaChatResponse> for AIResponse {
    fn from(resp: OllamaChatResponse) -> Self {
        let role = match resp.message.role.as_str() {
            "assistant" => AIRole::Assistant,
            "user" => AIRole::User,
            "system" => AIRole::System,
            _ => AIRole::Assistant,
        };
        AIResponse {
            id: String::new(),
            model: resp.model,
            choices: vec![AIChoice {
                index: 0,
                message: AIMessage {
                    role,
                    content: AIContent::Text(resp.message.content),
                    name: None,
                    tool_calls: vec![],
                    tool_call_id: None,
                },
                finish_reason: if resp.done { Some("stop".into()) } else { None },
            }],
            usage: match (resp.prompt_eval_count, resp.eval_count) {
                (Some(prompt), Some(completion)) => Some(AIUsage {
                    prompt_tokens: prompt,
                    completion_tokens: completion,
                    total_tokens: prompt + completion,
                }),
                _ => None,
            },
            created: None,
            extra: Default::default(),
        }
    }
}

pub struct OllamaAdapter;

#[async_trait]
impl FormatAdapter for OllamaAdapter {
    fn name(&self) -> &'static str {
        "ollama"
    }

    fn parse_request(&self, body: &[u8]) -> Result<AIRequest, AIError> {
        let req: OllamaChatRequest =
            serde_json::from_slice(body).map_err(|e| AIError::FormatParse {
                format: "ollama".into(),
                message: e.to_string(),
            })?;
        Ok(AIRequest::from(req))
    }

    fn parse_response(&self, body: &[u8]) -> Result<AIResponse, AIError> {
        let resp: OllamaChatResponse =
            serde_json::from_slice(body).map_err(|e| AIError::FormatParse {
                format: "ollama".into(),
                message: e.to_string(),
            })?;
        Ok(AIResponse::from(resp))
    }

    fn serialize_response(&self, response: &AIResponse) -> Result<Vec<u8>, AIError> {
        let resp = OllamaChatResponse::from(response);
        serde_json::to_vec(&resp).map_err(|e| AIError::FormatSerialize {
            format: "ollama".into(),
            message: e.to_string(),
        })
    }

    fn serialize_stream_chunk(&self, chunk: &AIStreamChunk) -> Result<String, AIError> {
        let first = chunk.choices.first();
        let content = first.and_then(|c| c.delta.content.as_deref()).unwrap_or("");
        let done = first.and_then(|c| c.finish_reason.as_deref()).is_some();

        let resp = serde_json::json!({
            "model": chunk.model,
            "created_at": "",
            "message": {"role": "assistant", "content": content},
            "done": done
        });

        let json = serde_json::to_string(&resp).map_err(|e| AIError::FormatSerialize {
            format: "ollama".into(),
            message: e.to_string(),
        })?;
        Ok(format!("{json}\n"))
    }

    fn error_response(&self, _status: u16, message: &str) -> Result<Vec<u8>, AIError> {
        let error = serde_json::json!({
            "error": message
        });
        #[allow(clippy::unwrap_used)]
        Ok(serde_json::to_vec(&error).unwrap())
    }
}
