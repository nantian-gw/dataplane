use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::error::AIError;
use crate::format::FormatAdapter;
use crate::format::ir::*;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct AnthropicMessageRequest {
    model: String,
    #[serde(default)]
    system: Option<AnthropicSystemContent>,
    messages: Vec<AnthropicMessage>,
    max_tokens: u32,
    #[serde(default)]
    stop_sequences: Option<Vec<String>>,
    #[serde(default)]
    stream: bool,
    #[serde(default)]
    temperature: Option<f32>,
    #[serde(default)]
    top_p: Option<f32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
enum AnthropicSystemContent {
    Text(String),
    Blocks(Vec<AnthropicContentBlock>),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct AnthropicMessage {
    role: String,
    content: AnthropicContent,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
enum AnthropicContent {
    Text(String),
    Blocks(Vec<AnthropicContentBlock>),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct AnthropicContentBlock {
    #[serde(rename = "type")]
    block_type: String,
    text: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct AnthropicMessageResponse {
    id: String,
    #[serde(rename = "type")]
    msg_type: String,
    role: String,
    model: String,
    content: Vec<AnthropicContentBlock>,
    stop_reason: Option<String>,
    usage: Option<AnthropicUsage>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct AnthropicUsage {
    input_tokens: u64,
    output_tokens: u64,
}

fn system_to_message(system: &AnthropicSystemContent) -> AIMessage {
    let content = match system {
        AnthropicSystemContent::Text(text) => AIContent::Text(text.clone()),
        AnthropicSystemContent::Blocks(blocks) => blocks_to_content(blocks.clone()),
    };
    AIMessage {
        role: AIRole::System,
        content,
        name: None,
        tool_calls: vec![],
        tool_call_id: None,
    }
}

fn blocks_to_content(blocks: Vec<AnthropicContentBlock>) -> AIContent {
    let texts: Vec<String> = blocks.into_iter().filter_map(|b| b.text).collect();
    match texts.as_slice() {
        [] => AIContent::None,
        [text] => AIContent::Text(text.clone()),
        _ => AIContent::Text(texts.join("\n")),
    }
}

fn serialize_message_stop_event() -> Result<String, AIError> {
    #[derive(Serialize)]
    struct MessageStop {
        #[serde(rename = "type")]
        event_type: String,
    }

    serde_json::to_string(&MessageStop {
        event_type: "message_stop".into(),
    })
    .map_err(|e| AIError::FormatSerialize {
        format: "anthropic".into(),
        message: e.to_string(),
    })
}

impl From<AnthropicContent> for AIContent {
    fn from(content: AnthropicContent) -> Self {
        match content {
            AnthropicContent::Text(text) => AIContent::Text(text),
            AnthropicContent::Blocks(blocks) => blocks_to_content(blocks),
        }
    }
}

impl From<AnthropicMessage> for AIMessage {
    fn from(msg: AnthropicMessage) -> Self {
        let role = match msg.role.as_str() {
            "user" => AIRole::User,
            "assistant" => AIRole::Assistant,
            _ => AIRole::User,
        };
        AIMessage {
            role,
            content: AIContent::from(msg.content),
            name: None,
            tool_calls: vec![],
            tool_call_id: None,
        }
    }
}

impl From<AnthropicMessageRequest> for AIRequest {
    fn from(req: AnthropicMessageRequest) -> Self {
        let mut messages: Vec<AIMessage> = Vec::new();

        if let Some(ref system) = req.system {
            messages.push(system_to_message(system));
        }

        messages.extend(req.messages.into_iter().map(AIMessage::from));

        AIRequest {
            model: req.model,
            messages,
            temperature: req.temperature,
            max_tokens: Some(req.max_tokens),
            top_p: req.top_p,
            stop: req.stop_sequences.unwrap_or_default(),
            stream: req.stream,
            user: None,
            extra: Default::default(),
        }
    }
}

impl From<&AIResponse> for AnthropicMessageResponse {
    fn from(resp: &AIResponse) -> Self {
        let content = resp
            .choices
            .iter()
            .map(|c| match &c.message.content {
                AIContent::Text(text) => AnthropicContentBlock {
                    block_type: "text".into(),
                    text: Some(text.clone()),
                },
                _ => AnthropicContentBlock {
                    block_type: "text".into(),
                    text: Some("[content]".into()),
                },
            })
            .collect();

        let role = match resp.choices.first().map(|c| c.message.role) {
            Some(AIRole::Assistant) => "assistant",
            _ => "assistant",
        };

        let stop_reason = resp.choices.first().and_then(|c| c.finish_reason.clone());

        AnthropicMessageResponse {
            id: resp.id.clone(),
            msg_type: "message".into(),
            role: role.into(),
            model: resp.model.clone(),
            content,
            stop_reason,
            usage: resp.usage.as_ref().map(|u| AnthropicUsage {
                input_tokens: u.prompt_tokens,
                output_tokens: u.completion_tokens,
            }),
        }
    }
}

impl From<AnthropicMessageResponse> for AIResponse {
    fn from(resp: AnthropicMessageResponse) -> Self {
        let content = resp
            .content
            .into_iter()
            .map(|b| {
                let text = b.text.unwrap_or_default();
                AIContent::Text(text)
            })
            .collect::<Vec<_>>()
            .first()
            .cloned()
            .unwrap_or(AIContent::None);

        let role = match resp.role.as_str() {
            "assistant" => AIRole::Assistant,
            "user" => AIRole::User,
            _ => AIRole::Assistant,
        };

        AIResponse {
            id: resp.id,
            model: resp.model,
            choices: vec![AIChoice {
                index: 0,
                message: AIMessage {
                    role,
                    content,
                    name: None,
                    tool_calls: vec![],
                    tool_call_id: None,
                },
                finish_reason: resp.stop_reason,
            }],
            usage: resp.usage.map(|u| AIUsage {
                prompt_tokens: u.input_tokens,
                completion_tokens: u.output_tokens,
                total_tokens: u.input_tokens + u.output_tokens,
            }),
            created: None,
            extra: Default::default(),
        }
    }
}

pub struct AnthropicAdapter;

#[async_trait]
impl FormatAdapter for AnthropicAdapter {
    fn name(&self) -> &'static str {
        "anthropic"
    }

    fn parse_request(&self, body: &[u8]) -> Result<AIRequest, AIError> {
        let req: AnthropicMessageRequest =
            serde_json::from_slice(body).map_err(|e| AIError::FormatParse {
                format: "anthropic".into(),
                message: e.to_string(),
            })?;
        Ok(AIRequest::from(req))
    }

    fn parse_response(&self, body: &[u8]) -> Result<AIResponse, AIError> {
        let resp: AnthropicMessageResponse =
            serde_json::from_slice(body).map_err(|e| AIError::FormatParse {
                format: "anthropic".into(),
                message: e.to_string(),
            })?;
        Ok(AIResponse::from(resp))
    }

    fn serialize_response(&self, response: &AIResponse) -> Result<Vec<u8>, AIError> {
        let resp = AnthropicMessageResponse::from(response);
        serde_json::to_vec(&resp).map_err(|e| AIError::FormatSerialize {
            format: "anthropic".into(),
            message: e.to_string(),
        })
    }

    fn serialize_stream_chunk(&self, chunk: &AIStreamChunk) -> Result<String, AIError> {
        let delta = &chunk.choices.first().map(|c| &c.delta);
        let content = delta.and_then(|d| d.content.as_deref()).unwrap_or("");
        let finish = chunk
            .choices
            .first()
            .and_then(|c| c.finish_reason.as_deref());

        #[derive(Serialize)]
        struct ContentBlockDelta {
            #[serde(rename = "type")]
            event_type: String,
            index: u32,
            delta: TextDelta,
        }

        #[derive(Serialize)]
        struct TextDelta {
            #[serde(rename = "type")]
            delta_type: String,
            text: String,
        }

        let delta_event = ContentBlockDelta {
            event_type: "content_block_delta".into(),
            index: 0,
            delta: TextDelta {
                delta_type: "text_delta".into(),
                text: content.into(),
            },
        };

        let json = serde_json::to_string(&delta_event).map_err(|e| AIError::FormatSerialize {
            format: "anthropic".into(),
            message: e.to_string(),
        })?;

        if finish.is_some() {
            let stop_json = serialize_message_stop_event()?;
            let header = "event: content_block_delta\ndata: ";
            let mid = "\n\nevent: message_stop\ndata: ";
            let mut buf = String::with_capacity(header.len() + json.len() + mid.len() + stop_json.len() + 2);
            buf.push_str(header);
            buf.push_str(&json);
            buf.push_str(mid);
            buf.push_str(&stop_json);
            buf.push_str("\n\n");
            Ok(buf)
        } else {
            let header = "event: content_block_delta\ndata: ";
            let mut buf = String::with_capacity(header.len() + json.len() + 2);
            buf.push_str(header);
            buf.push_str(&json);
            buf.push_str("\n\n");
            Ok(buf)
        }
    }

    fn error_response(&self, _status: u16, message: &str) -> Result<Vec<u8>, AIError> {
        let error = serde_json::json!({
            "type": "error",
            "error": {
                "type": "invalid_request_error",
                "message": message
            }
        });
        match serde_json::to_vec(&error) {
            Ok(body) => Ok(body),
            Err(_) => Ok(
                br#"{"type":"error","error":{"type":"invalid_request_error","message":"internal error"}}"#
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
    fn blocks_to_content_single_text_returns_text() {
        let content = blocks_to_content(vec![AnthropicContentBlock {
            block_type: "text".into(),
            text: Some("hello".into()),
        }]);

        assert!(matches!(content, AIContent::Text(ref text) if text == "hello"));
    }

    #[test]
    fn serialize_message_stop_event_returns_json() {
        let json = serialize_message_stop_event().expect("message stop event should serialize");
        let value: Value = serde_json::from_str(&json).expect("valid json");
        assert_eq!(value["type"], "message_stop");
    }

    #[test]
    fn anthropic_error_response_returns_json_body() {
        let body = AnthropicAdapter
            .error_response(400, "bad request")
            .expect("anthropic error response");
        let value: Value = serde_json::from_slice(&body).expect("valid json");

        assert_eq!(value["type"], "error");
        assert_eq!(value["error"]["message"], "bad request");
    }
}
