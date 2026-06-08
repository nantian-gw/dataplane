use std::sync::Arc;

pub type SharedApplyStageRecorder = Arc<dyn ApplyStageRecorder>;

pub trait ApplyStageRecorder: Send + Sync {
    fn observe_apply_stage_duration(&self, stage: &str, duration_ms: u64);
}
