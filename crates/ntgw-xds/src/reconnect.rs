use std::time::Duration;

use tracing::{debug, warn};

use crate::TransportOptions;
use crate::error::XdsError;

#[derive(Debug, Clone)]
pub struct ReconnectBackoff {
    initial: Duration,
    max: Duration,
    attempt: u32,
}

impl ReconnectBackoff {
    pub fn new(options: &TransportOptions) -> Self {
        Self {
            initial: options.initial_reconnect_backoff,
            max: options
                .max_reconnect_backoff
                .max(options.initial_reconnect_backoff),
            attempt: 0,
        }
    }

    pub fn reset(&mut self) {
        self.attempt = 0;
    }

    pub fn next_delay(&mut self) -> Duration {
        let shift = self.attempt.min(16);
        let multiplier = 1u64 << shift;
        let initial_ms = duration_millis(self.initial);
        let max_ms = duration_millis(self.max);
        let capped_ms = initial_ms.saturating_mul(multiplier).min(max_ms).max(1);
        self.attempt = self.attempt.saturating_add(1);

        let floor_ms = (capped_ms / 2).max(1);
        let spread_ms = capped_ms.saturating_sub(floor_ms);
        let jitter_ms = if spread_ms == 0 {
            0
        } else {
            deterministic_jitter(self.attempt, spread_ms + 1)
        };
        Duration::from_millis(floor_ms.saturating_add(jitter_ms))
    }
}

pub(crate) fn log_duplicate_snapshot_skipped(version: &str) {
    debug!(version = %version, "skipped duplicate snapshot");
}

pub(crate) fn log_stream_failure_retry(error: &XdsError, delay: Duration) {
    if is_expected_stream_reconnect_error(error) {
        debug!(
            error = %error,
            reconnect_delay_ms = delay.as_millis(),
            "xds stream closed, retrying"
        );
        return;
    }

    warn!(
        error = %error,
        reconnect_delay_ms = delay.as_millis(),
        "xds stream failed, retrying"
    );
}

pub(crate) fn log_heartbeat_report_failure(node_id: &str, error: &tonic::Status) {
    if is_expected_heartbeat_error(error) {
        debug!(
            node_id = %node_id,
            error = %error,
            "dataplane heartbeat interrupted during xds reconnect"
        );
        return;
    }

    warn!(
        node_id = %node_id,
        error = %error,
        "failed to report dataplane heartbeat"
    );
}

pub(crate) fn retry_delay_after_stream_failure(
    backoff: &mut ReconnectBackoff,
    stream_established: bool,
) -> Duration {
    if stream_established {
        backoff.reset();
    }
    backoff.next_delay()
}

fn duration_millis(duration: Duration) -> u64 {
    duration.as_millis().try_into().unwrap_or(u64::MAX)
}

fn is_expected_stream_reconnect_error(error: &XdsError) -> bool {
    is_source_expected_heartbeat_error(error)
        || looks_like_expected_transport_close(error.to_string().as_str())
}

fn is_source_expected_heartbeat_error(err: &(dyn std::error::Error + 'static)) -> bool {
    if let Some(status) = err.downcast_ref::<tonic::Status>() {
        return is_expected_heartbeat_error(status);
    }
    let mut source = err.source();
    while let Some(src) = source {
        if let Some(status) = src.downcast_ref::<tonic::Status>() {
            return is_expected_heartbeat_error(status);
        }
        source = src.source();
    }
    false
}

fn is_expected_heartbeat_error(error: &tonic::Status) -> bool {
    error.code() == tonic::Code::Cancelled || looks_like_expected_transport_close(error.message())
}

fn looks_like_expected_transport_close(message: &str) -> bool {
    let message = message.to_ascii_lowercase();
    [
        "operation was canceled",
        "operation was cancelled",
        "error reading a body from connection",
        "connection reset by peer",
        "broken pipe",
        "channel closed",
        "connection closed",
    ]
    .iter()
    .any(|needle| message.contains(needle))
}

fn deterministic_jitter(attempt: u32, modulo: u64) -> u64 {
    let seed = (attempt as u64)
        .wrapping_mul(1_103_515_245)
        .wrapping_add(12_345);
    if modulo == 0 { 0 } else { seed % modulo }
}
