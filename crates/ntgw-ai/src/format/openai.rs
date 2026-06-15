use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::error::AIError;
use crate::format::FormatAdapter;
use crate::format::ir::*;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct OpenAIChatRequest {
    model: String,
    messages: Vec<OpenAIMessage>,
    #[serde(default)]
    temperature: Option<f32>,
    #[serde(default)]
    max_tokens: Option<u32>,
    #[serde(default)]
    top_p: Option<f32>,
    #[serde(default)]
    stop: Option<Vec<String>>,
    #[serde(default)]
    stream: bool,
    #[serde(default)]
    user: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct OpenAIMessage {
    role: String,
    content: Option<serde_json::Value>,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    tool_calls: Option<Vec<OpenAIToolCall>>,
    #[serde(default)]
    tool_call_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct OpenAIToolCall {
    id: String,
    #[serde(rename = "type")]
    call_type: String,
    function: OpenAIFunction,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct OpenAIFunction {
    name: String,
    arguments: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct OpenAIChatResponse {
    id: String,
    #[serde(default)]
    object: String,
    model: String,
    created: u64,
    choices: Vec<OpenAIChoice>,
    #[serde(default)]
    usage: Option<OpenAIUsage>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct OpenAIChoice {
    index: u32,
    message: OpenAIMessage,
    #[serde(default)]
    finish_reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct OpenAIUsage {
    prompt_tokens: u64,
    completion_tokens: u64,
    total_tokens: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct OpenAIChatStreamChunk {
    id: String,
    #[serde(default)]
    object: String,
    model: String,
    created: u64,
    choices: Vec<OpenAIStreamChoice>,
    #[serde(default)]
    usage: Option<OpenAIUsage>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct OpenAIStreamChoice {
    index: u32,
    delta: OpenAIDelta,
    #[serde(default)]
    finish_reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct OpenAIDelta {
    #[serde(default)]
    role: Option<String>,
    #[serde(default)]
    content: Option<String>,
    #[serde(default)]
    tool_calls: Option<Vec<OpenAIToolCall>>,
}

impl From<OpenAIMessage> for AIMessage {
    fn from(msg: OpenAIMessage) -> Self {
        let content = match msg.content {
            None => AIContent::None,
            Some(serde_json::Value::String(text)) => AIContent::Text(text),
            Some(serde_json::Value::Array(parts)) => {
                let parts: Vec<AIContentPart> = parts
                    .into_iter()
                    .filter_map(|p| serde_json::from_value(p).ok())
                    .collect();
                if parts.is_empty() {
                    AIContent::None
                } else {
                    AIContent::MultiPart(parts)
                }
            }
            Some(_) => AIContent::None,
        };

        let role = match msg.role.as_str() {
            "system" => AIRole::System,
            "user" => AIRole::User,
            "assistant" => AIRole::Assistant,
            "tool" => AIRole::Tool,
            _ => AIRole::User,
        };

        AIMessage {
            role,
            content,
            name: msg.name,
            tool_calls: msg
                .tool_calls
                .unwrap_or_default()
                .into_iter()
                .map(|tc| AIToolCall {
                    id: tc.id,
                    call_type: tc.call_type,
                    function: AIToolCallFunction {
                        name: tc.function.name,
                        arguments: tc.function.arguments,
                    },
                })
                .collect(),
            tool_call_id: msg.tool_call_id,
        }
    }
}

impl From<OpenAIChatRequest> for AIRequest {
    fn from(req: OpenAIChatRequest) -> Self {
        AIRequest {
            model: req.model,
            messages: req.messages.into_iter().map(AIMessage::from).collect(),
            temperature: req.temperature,
            max_tokens: req.max_tokens,
            top_p: req.top_p,
            stop: req.stop.unwrap_or_default(),
            stream: req.stream,
            user: req.user,
            extra: Default::default(),
        }
    }
}

fn openai_message_from_ir(msg: &AIMessage) -> Result<OpenAIMessage, AIError> {
    let role = role_to_str(msg.role);

    let content = match &msg.content {
        AIContent::Text(text) => Some(serde_json::Value::String(text.clone())),
        AIContent::MultiPart(parts) => Some(serde_json::to_value(parts).map_err(|e| {
            AIError::FormatSerialize {
                format: "openai".into(),
                message: e.to_string(),
            }
        })?),
        AIContent::None => None,
    };

    Ok(OpenAIMessage {
        role,
        content,
        name: msg.name.clone(),
        tool_calls: tool_calls_to_openai(&msg.tool_calls),
        tool_call_id: msg.tool_call_id.clone(),
    })
}

fn openai_response_from_ir(resp: &AIResponse) -> Result<OpenAIChatResponse, AIError> {
    let mut choices = Vec::with_capacity(resp.choices.len());
    for choice in &resp.choices {
        choices.push(OpenAIChoice {
            index: choice.index,
            message: openai_message_from_ir(&choice.message)?,
            finish_reason: choice.finish_reason.clone(),
        });
    }

    Ok(OpenAIChatResponse {
        id: resp.id.clone(),
        object: "chat.completion".to_string(),
        model: resp.model.clone(),
        created: resp.created.unwrap_or(0),
        choices,
        usage: resp.usage.as_ref().map(usage_to_openai),
    })
}

impl From<OpenAIChatResponse> for AIResponse {
    fn from(resp: OpenAIChatResponse) -> Self {
        AIResponse {
            id: resp.id,
            model: resp.model,
            choices: resp
                .choices
                .into_iter()
                .map(|c| AIChoice {
                    index: c.index,
                    message: AIMessage::from(c.message),
                    finish_reason: c.finish_reason,
                })
                .collect(),
            usage: resp.usage.map(|u| AIUsage {
                prompt_tokens: u.prompt_tokens,
                completion_tokens: u.completion_tokens,
                total_tokens: u.total_tokens,
            }),
            created: Some(resp.created),
            extra: Default::default(),
        }
    }
}

impl From<&AIStreamChunk> for OpenAIChatStreamChunk {
    fn from(chunk: &AIStreamChunk) -> Self {
        OpenAIChatStreamChunk {
            id: chunk.id.clone(),
            object: "chat.completion.chunk".to_string(),
            model: chunk.model.clone(),
            created: chunk.created.unwrap_or(0),
            choices: chunk
                .choices
                .iter()
                .map(|c| OpenAIStreamChoice {
                    index: c.index,
                    delta: OpenAIDelta {
                        role: c.delta.role.map(role_to_str),
                        content: c.delta.content.clone(),
                        tool_calls: tool_calls_to_openai(&c.delta.tool_calls),
                    },
                    finish_reason: c.finish_reason.clone(),
                })
                .collect(),
            usage: chunk.usage.as_ref().map(usage_to_openai),
        }
    }
}

fn role_to_str(role: AIRole) -> String {
    match role {
        AIRole::System => "system",
        AIRole::User => "user",
        AIRole::Assistant => "assistant",
        AIRole::Tool => "tool",
    }
    .to_string()
}

fn usage_to_openai(usage: &AIUsage) -> OpenAIUsage {
    OpenAIUsage {
        prompt_tokens: usage.prompt_tokens,
        completion_tokens: usage.completion_tokens,
        total_tokens: usage.total_tokens,
    }
}

fn tool_calls_to_openai(tool_calls: &[AIToolCall]) -> Option<Vec<OpenAIToolCall>> {
    if tool_calls.is_empty() {
        None
    } else {
        Some(
            tool_calls
                .iter()
                .map(|tc| OpenAIToolCall {
                    id: tc.id.clone(),
                    call_type: tc.call_type.clone(),
                    function: OpenAIFunction {
                        name: tc.function.name.clone(),
                        arguments: tc.function.arguments.clone(),
                    },
                })
                .collect(),
        )
    }
}

pub struct OpenAIAdapter;

#[async_trait]
impl FormatAdapter for OpenAIAdapter {
    fn name(&self) -> &'static str {
        "openai"
    }

    fn parse_request(&self, body: &[u8]) -> Result<AIRequest, AIError> {
        let req: OpenAIChatRequest =
            serde_json::from_slice(body).map_err(|e| AIError::FormatParse {
                format: "openai".into(),
                message: e.to_string(),
            })?;
        Ok(AIRequest::from(req))
    }

    fn parse_response(&self, body: &[u8]) -> Result<AIResponse, AIError> {
        let resp: OpenAIChatResponse =
            serde_json::from_slice(body).map_err(|e| AIError::FormatParse {
                format: "openai".into(),
                message: e.to_string(),
            })?;
        Ok(AIResponse::from(resp))
    }

    fn serialize_response(&self, response: &AIResponse) -> Result<Vec<u8>, AIError> {
        let openai_resp = openai_response_from_ir(response)?;
        serde_json::to_vec(&openai_resp).map_err(|e| AIError::FormatSerialize {
            format: "openai".into(),
            message: e.to_string(),
        })
    }

    fn serialize_stream_chunk(&self, chunk: &AIStreamChunk) -> Result<String, AIError> {
        let openai_chunk = OpenAIChatStreamChunk::from(chunk);
        let json = serde_json::to_string(&openai_chunk).map_err(|e| AIError::FormatSerialize {
            format: "openai".into(),
            message: e.to_string(),
        })?;
        Ok(format!("data: {json}\n\n"))
    }

    fn error_response(&self, status: u16, message: &str) -> Result<Vec<u8>, AIError> {
        let error = serde_json::json!({
            "error": {
                "message": message,
                "type": "invalid_request_error",
                "code": status
            }
        });
        match serde_json::to_vec(&error) {
            Ok(body) => Ok(body),
            Err(_) => Ok(
                br#"{"error":{"message":"internal error","type":"invalid_request_error","code":500}}"#
                    .to_vec(),
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::format::FormatAdapter;
    use serde_json::Value;

    #[test]
    fn multipart_message_conversion_returns_serializable_value() {
        let message = AIMessage {
            role: AIRole::Assistant,
            content: AIContent::MultiPart(vec![AIContentPart {
                content_type: "text".into(),
                text: Some("hello".into()),
                image_url: None,
            }]),
            name: None,
            tool_calls: vec![],
            tool_call_id: None,
        };

        let openai =
            openai_message_from_ir(&message).expect("multipart content should serialize");
        let json = serde_json::to_value(&openai).expect("serialize converted message");

        assert!(json["content"].is_array());
        assert_eq!(json["content"][0]["type"], "text");
        assert_eq!(json["content"][0]["text"], "hello");
    }

    #[test]
    fn openai_error_response_returns_json_body() {
        let body = OpenAIAdapter
            .error_response(429, "slow down")
            .expect("openai error response");
        let value: Value = serde_json::from_slice(&body).expect("valid json");

        assert_eq!(value["error"]["message"], "slow down");
        assert_eq!(value["error"]["code"], 429);
    }
}
