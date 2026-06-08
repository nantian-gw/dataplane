use serde_json::{Value, json};

use crate::admin::AppState;

use super::{super::runtime::format_runtime_failures, ListenerSignalSummary, SummaryValueInputs};

pub(in crate::admin::summary) struct WarningData {
    pub(in crate::admin::summary) categories: Vec<String>,
    pub(in crate::admin::summary) counts: Value,
    pub(in crate::admin::summary) messages: Vec<String>,
    pub(super) primary_category: String,
    pub(super) primary_message: String,
}

pub(in crate::admin::summary) fn build_warning_data(
    state: &AppState,
    inputs: &SummaryValueInputs<'_>,
    listener_signals: &ListenerSignalSummary,
) -> WarningData {
    let config = state.current_config();
    let mut messages = Vec::new();
    let mut categories = Vec::new();

    if inputs.session_persistence.active() && config.session_persistence_uses_ephemeral_secret {
        messages.push(
            "session persistence is active but the dataplane is using an ephemeral secret; configure sessionPersistence.secretKey or secretKeyFile for restart-stable multi-replica deployments"
                .to_string(),
        );
        categories.push("session-persistence-ephemeral-secret".to_string());
    }
    if inputs.runtime.http_last_reload_failure_version == inputs.snapshot.id
        && !inputs.runtime.http_last_reload_failure_message.is_empty()
    {
        messages.push(format!(
            "latest HTTP listener reload for snapshot {} failed: {}",
            inputs.runtime.http_last_reload_failure_version,
            format_runtime_failures(&inputs.runtime.http_current_failures)
        ));
        categories.push("runtime-http-reload-failure".to_string());
    }
    if inputs.runtime.tls_last_reload_failure_version == inputs.snapshot.id
        && !inputs.runtime.tls_last_reload_failure_message.is_empty()
    {
        messages.push(format!(
            "latest TLS listener reload for snapshot {} failed: {}",
            inputs.runtime.tls_last_reload_failure_version,
            format_runtime_failures(&inputs.runtime.tls_current_failures)
        ));
        categories.push("runtime-tls-reload-failure".to_string());
    }
    if inputs.runtime.stream_last_reload_failure_version == inputs.snapshot.id
        && !inputs.runtime.stream_last_reload_failure_message.is_empty()
    {
        messages.push(format!(
            "latest stream listener reload for snapshot {} failed: {}",
            inputs.runtime.stream_last_reload_failure_version,
            format_runtime_failures(&inputs.runtime.stream_current_failures)
        ));
        categories.push("runtime-stream-reload-failure".to_string());
    }
    if !listener_signals.pending_warning_names.is_empty() {
        messages.push(format!(
            "listeners still pending for snapshot {}: {}",
            inputs.snapshot.id,
            listener_signals.pending_warning_names.join(", ")
        ));
        categories.push("listener-pending".to_string());
    }
    if !listener_signals.rejected_warning_names.is_empty() {
        messages.push(format!(
            "listeners currently rejected for snapshot {}: {}",
            inputs.snapshot.id,
            listener_signals.rejected_warning_names.join(", ")
        ));
        categories.push("listener-rejected".to_string());
    }
    if !listener_signals.stale_warning_names.is_empty() {
        messages.push(format!(
            "listeners still serving last-good snapshot instead of {}: {}",
            inputs.snapshot.id,
            listener_signals.stale_warning_names.join(", ")
        ));
        categories.push("listener-stale".to_string());
    }
    if !listener_signals
        .unrecovered_failure_warning_names
        .is_empty()
    {
        messages.push(format!(
            "listeners with observed failures not yet recovered: {}",
            listener_signals
                .unrecovered_failure_warning_names
                .join(", ")
        ));
        categories.push("listener-unrecovered-failure".to_string());
    }

    let primary_category = categories.first().cloned().unwrap_or_default();
    let primary_message = messages.first().cloned().unwrap_or_default();
    let counts = json!({
        "sessionPersistenceEphemeralSecret": count_warning_category(&categories, "session-persistence-ephemeral-secret"),
        "runtimeHttpReloadFailure": count_warning_category(&categories, "runtime-http-reload-failure"),
        "runtimeTlsReloadFailure": count_warning_category(&categories, "runtime-tls-reload-failure"),
        "runtimeStreamReloadFailure": count_warning_category(&categories, "runtime-stream-reload-failure"),
        "listenerPending": count_warning_category(&categories, "listener-pending"),
        "listenerRejected": count_warning_category(&categories, "listener-rejected"),
        "listenerStale": count_warning_category(&categories, "listener-stale"),
        "listenerUnrecoveredFailure": count_warning_category(&categories, "listener-unrecovered-failure"),
    });

    WarningData {
        categories,
        counts,
        messages,
        primary_category,
        primary_message,
    }
}

fn count_warning_category(categories: &[String], expected: &str) -> usize {
    categories
        .iter()
        .filter(|category| category.as_str() == expected)
        .count()
}
