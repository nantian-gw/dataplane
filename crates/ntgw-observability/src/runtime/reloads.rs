use super::helpers::{epoch_seconds, record_listener_event};
use super::*;
use std::collections::BTreeSet;

impl RuntimeStats {
    pub fn observe_http_listener_reload_attempt(&self, version: &str) {
        self.inner
            .write()
            .unwrap_or_else(|err| err.into_inner())
            .http_last_reload_attempt_version = version.to_string();
    }

    pub fn observe_http_listener_reload_failure(&self, version: &str, listener: &str, error: &str) {
        self.observe_http_listener_reload_failures(
            version,
            &[RuntimeListenerFailure {
                listener: listener.to_string(),
                message: error.to_string(),
            }],
        );
    }

    pub fn observe_http_listener_reload_failures(
        &self,
        version: &str,
        failures: &[RuntimeListenerFailure],
    ) {
        self.observe_http_listener_reload_result(version, &[], &[], failures);
    }

    pub fn observe_http_listener_reload_result(
        &self,
        version: &str,
        successful_listeners: &[String],
        retained_listeners: &[String],
        failures: &[RuntimeListenerFailure],
    ) {
        self.http_listener_reload_failures
            .fetch_add(u64::from(!failures.is_empty()), Ordering::Relaxed);
        self.observe_http_listener_reload_attempt(version);
        let mut inner = self
            .inner
            .write()
            .unwrap_or_else(|err| err.into_inner());
        update_listener_progress(
            &mut inner.http_listener_progress,
            version,
            successful_listeners,
            retained_listeners,
            failures,
        );
        if failures.is_empty() {
            inner.http_last_good_reload_version = version.to_string();
            inner.http_last_reload_failure_version.clear();
            inner.http_last_reload_failure_listener.clear();
            inner.http_last_reload_failure_message.clear();
            inner.http_current_failures.clear();
            self.publish_apply_event(RuntimeApplyEvent {
                version: version.to_string(),
                plane: RuntimePlane::Http,
                outcome: RuntimeApplyOutcome::Applied,
                message: String::new(),
            });
            return;
        }
        inner.http_last_reload_failure_version = version.to_string();
        let first = failures.first().cloned().unwrap_or_default();
        let first_message = first.message.clone();
        inner.http_last_reload_failure_listener = first.listener;
        inner.http_last_reload_failure_message = first_message.clone();
        inner.http_current_failures = failures.to_vec();
        drop(inner);
        self.publish_apply_event(RuntimeApplyEvent {
            version: version.to_string(),
            plane: RuntimePlane::Http,
            outcome: RuntimeApplyOutcome::Rejected,
            message: first_message,
        });
    }

    pub fn observe_tls_listener_reload_attempt(&self, version: &str) {
        self.inner
            .write()
            .unwrap_or_else(|err| err.into_inner())
            .tls_last_reload_attempt_version = version.to_string();
    }

    pub fn observe_tls_listener_reload_failure(&self, version: &str, listener: &str, error: &str) {
        self.observe_tls_listener_reload_failures(
            version,
            &[RuntimeListenerFailure {
                listener: listener.to_string(),
                message: error.to_string(),
            }],
        );
    }

    pub fn observe_tls_listener_reload_failures(
        &self,
        version: &str,
        failures: &[RuntimeListenerFailure],
    ) {
        self.observe_tls_listener_reload_result(version, &[], &[], failures);
    }

    pub fn observe_tls_listener_reload_result(
        &self,
        version: &str,
        successful_listeners: &[String],
        retained_listeners: &[String],
        failures: &[RuntimeListenerFailure],
    ) {
        self.tls_listener_reload_failures
            .fetch_add(u64::from(!failures.is_empty()), Ordering::Relaxed);
        self.observe_tls_listener_reload_attempt(version);
        let mut inner = self
            .inner
            .write()
            .unwrap_or_else(|err| err.into_inner());
        update_listener_progress(
            &mut inner.tls_listener_progress,
            version,
            successful_listeners,
            retained_listeners,
            failures,
        );
        if failures.is_empty() {
            inner.tls_last_good_reload_version = version.to_string();
            inner.tls_last_reload_failure_version.clear();
            inner.tls_last_reload_failure_listener.clear();
            inner.tls_last_reload_failure_message.clear();
            inner.tls_current_failures.clear();
            self.publish_apply_event(RuntimeApplyEvent {
                version: version.to_string(),
                plane: RuntimePlane::Tls,
                outcome: RuntimeApplyOutcome::Applied,
                message: String::new(),
            });
            return;
        }
        inner.tls_last_reload_failure_version = version.to_string();
        let first = failures.first().cloned().unwrap_or_default();
        let first_message = first.message.clone();
        inner.tls_last_reload_failure_listener = first.listener;
        inner.tls_last_reload_failure_message = first_message.clone();
        inner.tls_current_failures = failures.to_vec();
        drop(inner);
        self.publish_apply_event(RuntimeApplyEvent {
            version: version.to_string(),
            plane: RuntimePlane::Tls,
            outcome: RuntimeApplyOutcome::Rejected,
            message: first_message,
        });
    }

    pub fn observe_stream_listener_reload_attempt(&self, version: &str) {
        self.inner
            .write()
            .unwrap_or_else(|err| err.into_inner())
            .stream_last_reload_attempt_version = version.to_string();
    }

    pub fn observe_stream_listener_reload_failure(
        &self,
        version: &str,
        listener: &str,
        error: &str,
    ) {
        self.observe_stream_listener_reload_failures(
            version,
            &[RuntimeListenerFailure {
                listener: listener.to_string(),
                message: error.to_string(),
            }],
        );
    }

    pub fn observe_stream_listener_reload_failures(
        &self,
        version: &str,
        failures: &[RuntimeListenerFailure],
    ) {
        self.observe_stream_listener_reload_result(version, &[], &[], failures);
    }

    pub fn observe_stream_listener_reload_result(
        &self,
        version: &str,
        successful_listeners: &[String],
        retained_listeners: &[String],
        failures: &[RuntimeListenerFailure],
    ) {
        self.stream_listener_reload_failures
            .fetch_add(u64::from(!failures.is_empty()), Ordering::Relaxed);
        self.observe_stream_listener_reload_attempt(version);
        let mut inner = self
            .inner
            .write()
            .unwrap_or_else(|err| err.into_inner());
        update_listener_progress(
            &mut inner.stream_listener_progress,
            version,
            successful_listeners,
            retained_listeners,
            failures,
        );
        if failures.is_empty() {
            inner.stream_last_good_reload_version = version.to_string();
            inner.stream_last_reload_failure_version.clear();
            inner.stream_last_reload_failure_listener.clear();
            inner.stream_last_reload_failure_message.clear();
            inner.stream_current_failures.clear();
            self.publish_apply_event(RuntimeApplyEvent {
                version: version.to_string(),
                plane: RuntimePlane::Stream,
                outcome: RuntimeApplyOutcome::Applied,
                message: String::new(),
            });
            return;
        }
        inner.stream_last_reload_failure_version = version.to_string();
        let first = failures.first().cloned().unwrap_or_default();
        let first_message = first.message.clone();
        inner.stream_last_reload_failure_listener = first.listener;
        inner.stream_last_reload_failure_message = first_message.clone();
        inner.stream_current_failures = failures.to_vec();
        drop(inner);
        self.publish_apply_event(RuntimeApplyEvent {
            version: version.to_string(),
            plane: RuntimePlane::Stream,
            outcome: RuntimeApplyOutcome::Rejected,
            message: first_message,
        });
    }

    fn publish_apply_event(&self, event: RuntimeApplyEvent) {
        let _ = self.apply_event_tx.send(Some(event));
    }
}

fn update_listener_progress(
    states: &mut BTreeMap<String, RuntimeListenerProgress>,
    version: &str,
    successful_listeners: &[String],
    retained_listeners: &[String],
    failures: &[RuntimeListenerFailure],
) {
    let now = epoch_seconds();
    let failed_listeners = failures
        .iter()
        .map(|failure| failure.listener.as_str())
        .collect::<BTreeSet<_>>();
    let successful_listeners = successful_listeners
        .iter()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();

    for &listener in &successful_listeners {
        let state = states.entry(listener.to_string()).or_default();
        state.attempts += 1;
        state.last_attempt_version = version.to_string();
        state.last_good_version = version.to_string();
        state.last_apply_unix_seconds = now;
        record_listener_event(
            state,
            RuntimeListenerEvent {
                status: "accepted".to_string(),
                version: version.to_string(),
                message: String::new(),
                unix_seconds: now,
            },
        );
    }

    for listener in retained_listeners {
        if successful_listeners.contains(listener.as_str())
            || failed_listeners.contains(listener.as_str())
        {
            continue;
        }

        let state = states.entry(listener.clone()).or_default();
        state.last_good_version = version.to_string();
        record_listener_event(
            state,
            RuntimeListenerEvent {
                status: "retained".to_string(),
                version: version.to_string(),
                message: String::new(),
                unix_seconds: now,
            },
        );
    }

    for failure in failures {
        let state = states.entry(failure.listener.clone()).or_default();
        state.attempts += 1;
        state.failures += 1;
        state.last_attempt_version = version.to_string();
        state.last_failure_version = version.to_string();
        state.last_failure_message = failure.message.clone();
        state.last_failure_unix_seconds = now;
        record_listener_event(
            state,
            RuntimeListenerEvent {
                status: "rejected".to_string(),
                version: version.to_string(),
                message: failure.message.clone(),
                unix_seconds: now,
            },
        );
    }
}
