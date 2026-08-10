use std::collections::BTreeMap;

use super::client::LangfuseClient;
use super::types::{
    DatasetCreatePayload, ExperimentCreatePayload, LangfuseGenerationPayload, LangfuseScorePayload,
    LangfuseTracePayload, PromptTemplate,
};
use crate::error::AIError;

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
        usage: super::types::LangfuseUsage {
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
        usage: super::types::LangfuseUsage {
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
