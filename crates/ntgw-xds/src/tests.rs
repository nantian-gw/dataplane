use super::status::required_runtime_names;
use super::{
    ReconnectBackoff, RuntimeApplyRequirements, SNAPSHOT_APPLIED_MESSAGE,
    SNAPSHOT_REJECTED_MESSAGE_PREFIX, TransportOptions, WAITING_FOR_SNAPSHOT_MESSAGE,
    build_status_report, discovery_ack, discovery_nack, discovery_open,
    preflight_snapshot_before_swap, retry_delay_after_stream_failure, should_apply_snapshot,
    snapshot_runtime_apply_requirements, snapshot_version_from_response,
    wait_for_runtime_apply_result, wait_for_stream_message,
};
use crate::ClientStats;
use crate::features::{
    canonicalize_supported_features, preflight_required_features, supported_features,
};
use ntgw_ir::{Listener, Snapshot};
use ntgw_observability::RuntimeStats;
use ntgw_proto::gateway::control::v1::{ConfigSnapshot, DiscoveryResultStatus};
use std::{
    io,
    sync::{Arc, Mutex},
    time::{Duration, SystemTime},
};
use tracing::Level;
use tracing_subscriber::fmt::MakeWriter;

mod logging;
mod reconnect_backoff;
mod runtime_apply;
mod status_reports;

pub(crate) fn run_exact_discovery_messages_include_supported_features() {
    status_reports::run_discovery_messages_include_supported_features();
}

pub(crate) fn run_exact_preflight_required_features_reports_sorted_missing_features() {
    status_reports::run_preflight_required_features_reports_sorted_missing_features();
}

pub(crate) fn run_exact_preflight_required_features_accepts_supported_snapshot() {
    status_reports::run_preflight_required_features_accepts_supported_snapshot();
}

pub(crate) fn run_exact_preflight_rejection_keeps_last_good_snapshot() {
    runtime_apply::run_preflight_rejection_keeps_last_good_snapshot();
}

#[derive(Clone, Default)]
struct SharedTestWriter {
    buffer: Arc<Mutex<Vec<u8>>>,
}

struct SharedWriterGuard {
    buffer: Arc<Mutex<Vec<u8>>>,
}

impl SharedTestWriter {
    fn contents(&self) -> String {
        String::from_utf8(self.buffer.lock().expect("writer buffer").clone())
            .expect("writer output should be valid utf-8")
    }
}

impl<'a> MakeWriter<'a> for SharedTestWriter {
    type Writer = SharedWriterGuard;

    fn make_writer(&'a self) -> Self::Writer {
        SharedWriterGuard {
            buffer: self.buffer.clone(),
        }
    }
}

impl io::Write for SharedWriterGuard {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.buffer
            .lock()
            .expect("writer buffer")
            .extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
