use std::collections::BTreeMap;

use ntgw_ai::observability::langfuse::LangfuseClient;

#[test]
fn test_noop_client_construction() {
    let client = LangfuseClient::noop_client();
    assert!(!client.enabled(), "noop client should be disabled");
}

#[test]
fn test_client_construction_with_credentials() {
    let client = LangfuseClient::new("pk-test", "sk-test", "https://example.com");
    assert!(
        client.enabled(),
        "client should be enabled when public_key is non-empty"
    );
}

#[tokio::test]
async fn test_noop_ingest_trace() {
    let client = LangfuseClient::noop_client();
    let mut metadata = BTreeMap::new();
    metadata.insert("env".to_string(), "test".to_string());

    let result = client
        .ingest_trace("trace-abc", Some("user-1"), Some("sess-1"), &metadata)
        .await;
    assert!(result.is_ok(), "noop trace ingestion should succeed");
}

#[tokio::test]
async fn test_noop_ingest_generation() {
    let client = LangfuseClient::noop_client();

    let mut metadata = BTreeMap::new();
    metadata.insert("source".to_string(), "gateway".to_string());

    let result = client
        .ingest_generation(
            "trace-abc",
            "gpt-4",
            100,
            50,
            500,
            &serde_json::json!({"prompt": "hello"}),
            &serde_json::json!({"completion": "world"}),
            &metadata,
        )
        .await;
    assert!(result.is_ok(), "noop generation ingestion should succeed");
}

#[tokio::test]
async fn test_noop_client_with_empty_keys_is_equivalent_to_noop() {
    let empty_client = LangfuseClient::new("", "", "");
    let noop_client = LangfuseClient::noop_client();

    assert_eq!(
        empty_client.enabled(),
        noop_client.enabled(),
        "both should be disabled"
    );

    let empty_result = empty_client
        .ingest_trace("t", None, None, &BTreeMap::new())
        .await;
    let noop_result = noop_client
        .ingest_trace("t", None, None, &BTreeMap::new())
        .await;

    assert!(empty_result.is_ok());
    assert!(noop_result.is_ok());
}
