use std::{
    collections::BTreeMap,
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Arc, RwLock,
    },
    time::{SystemTime, UNIX_EPOCH},
};

use ntgw_observability::ApplyStageRecorder;

pub type SharedClientStats = Arc<ClientStats>;
pub const XDS_APPLY_STAGE_MS_BUCKET_BOUNDS: [u64; 11] =
    [1, 5, 10, 25, 50, 100, 250, 500, 1_000, 2_500, 5_000];
const XDS_APPLY_STAGE_MS_BUCKET_COUNT: usize = XDS_APPLY_STAGE_MS_BUCKET_BOUNDS.len() + 1;

#[derive(Debug, Default)]
pub struct ClientStats {
    connect_failures: AtomicU64,
    stream_connected: AtomicBool,
    last_connect_failure_unix_seconds: AtomicU64,
    last_control_plane_contact_unix_seconds: AtomicU64,
    last_connect_error: RwLock<String>,
    last_apply_unix_seconds: AtomicU64,
    last_nack_message: RwLock<String>,
    last_nack_version: RwLock<String>,
    last_stream_error: RwLock<String>,
    last_stream_failure_unix_seconds: AtomicU64,
    snapshots_skipped: AtomicU64,
    snapshots_applied: AtomicU64,
    snapshots_nacked: AtomicU64,
    stream_failures: AtomicU64,
    last_snapshot_version: RwLock<String>,
    apply_stage_ms_histograms: RwLock<BTreeMap<String, XdsApplyStageHistogramState>>,
}

#[derive(Debug, Clone, Default)]
pub struct ClientStatsSnapshot {
    pub connect_failures: u64,
    pub stream_connected: bool,
    pub last_connect_error: String,
    pub last_connect_failure_unix_seconds: u64,
    pub last_control_plane_contact_unix_seconds: u64,
    pub last_apply_unix_seconds: u64,
    pub last_nack_message: String,
    pub last_nack_version: String,
    pub last_snapshot_version: String,
    pub last_stream_error: String,
    pub last_stream_failure_unix_seconds: u64,
    pub snapshots_skipped: u64,
    pub snapshots_applied: u64,
    pub snapshots_nacked: u64,
    pub stream_failures: u64,
    pub apply_stage_ms_histograms: Vec<XdsApplyStageHistogram>,
}

#[derive(Debug, Clone, Default)]
pub struct XdsApplyStageHistogram {
    pub stage: String,
    pub buckets: Vec<XdsApplyStageHistogramBucket>,
    pub sum: u64,
    pub count: u64,
}

#[derive(Debug, Clone, Default)]
pub struct XdsApplyStageHistogramBucket {
    pub le: String,
    pub cumulative_count: u64,
}

#[derive(Debug, Clone)]
struct XdsApplyStageHistogramState {
    buckets: [u64; XDS_APPLY_STAGE_MS_BUCKET_COUNT],
    sum: u64,
    count: u64,
}

impl Default for XdsApplyStageHistogramState {
    fn default() -> Self {
        Self {
            buckets: [0; XDS_APPLY_STAGE_MS_BUCKET_COUNT],
            sum: 0,
            count: 0,
        }
    }
}

impl ClientStats {
    pub fn shared() -> SharedClientStats {
        Arc::new(Self::default())
    }

    pub fn observe_connect_failure(&self) {
        self.observe_connect_failure_with_error("");
    }

    pub fn observe_connect_failure_with_error(&self, error: &str) {
        self.connect_failures.fetch_add(1, Ordering::Relaxed);
        self.last_connect_failure_unix_seconds
            .store(epoch_seconds(), Ordering::Relaxed);
        *self
            .last_connect_error
            .write()
            .unwrap_or_else(|err| err.into_inner()) = error.to_string();
    }

    pub fn observe_stream_connected(&self) {
        self.stream_connected.store(true, Ordering::Relaxed);
        self.observe_control_plane_contact();
    }

    pub fn observe_control_plane_contact(&self) {
        self.last_control_plane_contact_unix_seconds
            .store(epoch_seconds(), Ordering::Relaxed);
    }

    pub fn observe_stream_disconnected(&self) {
        self.stream_connected.store(false, Ordering::Relaxed);
    }

    pub fn observe_stream_failure(&self) {
        self.observe_stream_failure_with_error("");
    }

    pub fn observe_stream_failure_with_error(&self, error: &str) {
        self.observe_stream_disconnected();
        self.stream_failures.fetch_add(1, Ordering::Relaxed);
        self.last_stream_failure_unix_seconds
            .store(epoch_seconds(), Ordering::Relaxed);
        *self
            .last_stream_error
            .write()
            .unwrap_or_else(|err| err.into_inner()) = error.to_string();
    }

    pub fn observe_snapshot_applied(&self, version: &str) {
        self.snapshots_applied.fetch_add(1, Ordering::Relaxed);
        self.last_apply_unix_seconds
            .store(epoch_seconds(), Ordering::Relaxed);
        *self
            .last_snapshot_version
            .write()
            .unwrap_or_else(|err| err.into_inner()) = version.to_string();
        self.clear_snapshot_nack();
    }

    pub fn observe_snapshot_skipped(&self) {
        self.snapshots_skipped.fetch_add(1, Ordering::Relaxed);
    }

    pub fn observe_snapshot_nacked(&self, version: &str, message: &str) {
        self.snapshots_nacked.fetch_add(1, Ordering::Relaxed);
        *self
            .last_nack_version
            .write()
            .unwrap_or_else(|err| err.into_inner()) = version.to_string();
        *self
            .last_nack_message
            .write()
            .unwrap_or_else(|err| err.into_inner()) = message.to_string();
    }

    pub fn observe_apply_stage_duration(&self, stage: &str, duration_ms: u64) {
        let stage = stage.trim();
        if stage.is_empty() {
            return;
        }

        let mut histograms = self
            .apply_stage_ms_histograms
            .write()
            .unwrap_or_else(|err| err.into_inner());
        let histogram = histograms.entry(stage.to_string()).or_default();
        histogram.count = histogram.count.saturating_add(1);
        histogram.sum = histogram.sum.saturating_add(duration_ms);
        let bucket = xds_apply_stage_ms_bucket_index(duration_ms);
        histogram.buckets[bucket] = histogram.buckets[bucket].saturating_add(1);
    }

    pub fn snapshot(&self) -> ClientStatsSnapshot {
        ClientStatsSnapshot {
            connect_failures: self.connect_failures.load(Ordering::Relaxed),
            stream_connected: self.stream_connected.load(Ordering::Relaxed),
            last_connect_error: self
                .last_connect_error
                .read()
                .unwrap_or_else(|err| err.into_inner())
                .clone(),
            last_connect_failure_unix_seconds: self
                .last_connect_failure_unix_seconds
                .load(Ordering::Relaxed),
            last_control_plane_contact_unix_seconds: self
                .last_control_plane_contact_unix_seconds
                .load(Ordering::Relaxed),
            last_apply_unix_seconds: self.last_apply_unix_seconds.load(Ordering::Relaxed),
            last_nack_message: self
                .last_nack_message
                .read()
                .unwrap_or_else(|err| err.into_inner())
                .clone(),
            last_nack_version: self
                .last_nack_version
                .read()
                .unwrap_or_else(|err| err.into_inner())
                .clone(),
            last_snapshot_version: self
                .last_snapshot_version
                .read()
                .unwrap_or_else(|err| err.into_inner())
                .clone(),
            last_stream_error: self
                .last_stream_error
                .read()
                .unwrap_or_else(|err| err.into_inner())
                .clone(),
            last_stream_failure_unix_seconds: self
                .last_stream_failure_unix_seconds
                .load(Ordering::Relaxed),
            snapshots_skipped: self.snapshots_skipped.load(Ordering::Relaxed),
            snapshots_applied: self.snapshots_applied.load(Ordering::Relaxed),
            snapshots_nacked: self.snapshots_nacked.load(Ordering::Relaxed),
            stream_failures: self.stream_failures.load(Ordering::Relaxed),
            apply_stage_ms_histograms: apply_stage_histograms_snapshot(
                &self
                    .apply_stage_ms_histograms
                    .read()
                    .unwrap_or_else(|err| err.into_inner()),
            ),
        }
    }

    fn clear_snapshot_nack(&self) {
        self.last_nack_version
            .write()
            .unwrap_or_else(|err| err.into_inner())
            .clear();
        self.last_nack_message
            .write()
            .unwrap_or_else(|err| err.into_inner())
            .clear();
    }
}

impl ApplyStageRecorder for ClientStats {
    fn observe_apply_stage_duration(&self, stage: &str, duration_ms: u64) {
        ClientStats::observe_apply_stage_duration(self, stage, duration_ms);
    }
}

fn xds_apply_stage_ms_bucket_index(duration_ms: u64) -> usize {
    XDS_APPLY_STAGE_MS_BUCKET_BOUNDS
        .iter()
        .position(|bound| duration_ms <= *bound)
        .unwrap_or(XDS_APPLY_STAGE_MS_BUCKET_BOUNDS.len())
}

fn apply_stage_histograms_snapshot(
    histograms: &BTreeMap<String, XdsApplyStageHistogramState>,
) -> Vec<XdsApplyStageHistogram> {
    histograms
        .iter()
        .map(|(stage, histogram)| XdsApplyStageHistogram {
            stage: stage.clone(),
            buckets: apply_stage_bucket_snapshot(&histogram.buckets),
            sum: histogram.sum,
            count: histogram.count,
        })
        .collect()
}

fn apply_stage_bucket_snapshot(
    buckets: &[u64; XDS_APPLY_STAGE_MS_BUCKET_COUNT],
) -> Vec<XdsApplyStageHistogramBucket> {
    let mut cumulative = 0u64;
    let mut out = Vec::with_capacity(XDS_APPLY_STAGE_MS_BUCKET_COUNT);
    for (index, bound) in XDS_APPLY_STAGE_MS_BUCKET_BOUNDS.iter().enumerate() {
        cumulative = cumulative.saturating_add(buckets[index]);
        out.push(XdsApplyStageHistogramBucket {
            le: bound.to_string(),
            cumulative_count: cumulative,
        });
    }
    cumulative = cumulative.saturating_add(buckets[XDS_APPLY_STAGE_MS_BUCKET_BOUNDS.len()]);
    out.push(XdsApplyStageHistogramBucket {
        le: "+Inf".to_string(),
        cumulative_count: cumulative,
    });
    out
}

fn epoch_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|value| value.as_secs())
        .unwrap_or_default()
}
