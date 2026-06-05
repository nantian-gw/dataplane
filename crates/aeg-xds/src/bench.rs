use serde::{Deserialize, Serialize};

use super::*;
use aeg_ir::Listener;

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct ApplyBenchConfig {
    pub include_http: bool,
    pub include_stream: bool,
}

impl Default for ApplyBenchConfig {
    fn default() -> Self {
        Self {
            include_http: true,
            include_stream: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApplyOutcome {
    pub version: String,
    pub ready: bool,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FallbackOutcome {
    pub rejected_version: String,
    pub last_good_version: String,
    pub ready: bool,
    pub message: String,
    pub apply_error: String,
}

#[derive(Debug, Clone)]
pub struct ReloadBench {
    snapshot: SharedSnapshot,
    runtime: SharedRuntimeStats,
    requirements: RuntimeApplyRequirements,
}

impl ReloadBench {
    pub fn new(config: ApplyBenchConfig) -> Self {
        let snapshot = Snapshot::shared();
        let initial = Snapshot {
            listeners: bench_listeners(config),
            ..Snapshot::default()
        };
        let requirements = snapshot_runtime_apply_requirements(&initial);
        *snapshot.write() = initial;

        Self {
            snapshot,
            runtime: aeg_observability::RuntimeStats::shared(),
            requirements,
        }
    }

    pub async fn apply_success(&self, version: &str) -> std::result::Result<ApplyOutcome, String> {
        self.snapshot.write().id = version.to_string();
        if self.requirements.http {
            self.runtime.observe_http_listener_reload_success(version);
        }
        if self.requirements.stream {
            self.runtime.observe_stream_listener_reload_success(version);
        }

        wait_for_runtime_apply_result(
            self.runtime.clone(),
            version,
            self.requirements,
            &TransportOptions::default(),
        )
        .await?;

        let report = build_status_report("bench-node", &self.snapshot, &self.runtime, true);
        Ok(ApplyOutcome {
            version: report.version,
            ready: report.ready,
            message: report.message,
        })
    }

    pub async fn apply_failure_with_last_good(
        &self,
        last_good: &str,
        rejected: &str,
    ) -> FallbackOutcome {
        if self.requirements.http {
            self.runtime.observe_http_listener_reload_success(last_good);
        }
        if self.requirements.stream {
            self.runtime
                .observe_stream_listener_reload_success(last_good);
        }

        self.snapshot.write().id = rejected.to_string();
        if self.requirements.http {
            self.runtime.observe_http_listener_reload_failure(
                rejected,
                "bench/http",
                "bind conflict",
            );
        } else if self.requirements.stream {
            self.runtime.observe_stream_listener_reload_failure(
                rejected,
                "bench/stream",
                "bind conflict",
            );
        }

        let apply_error = wait_for_runtime_apply_result(
            self.runtime.clone(),
            rejected,
            self.requirements,
            &TransportOptions::default(),
        )
        .await
        .err()
        .unwrap_or_default();
        let report = build_status_report("bench-node", &self.snapshot, &self.runtime, true);
        let snapshot = self.runtime.snapshot();
        let last_good_version = if self.requirements.http {
            snapshot.http_last_good_reload_version
        } else {
            snapshot.stream_last_good_reload_version
        };

        FallbackOutcome {
            rejected_version: rejected.to_string(),
            last_good_version,
            ready: report.ready,
            message: report.message,
            apply_error,
        }
    }
}

fn bench_listeners(config: ApplyBenchConfig) -> Vec<Listener> {
    let mut listeners = Vec::new();

    if config.include_http {
        listeners.push(Listener {
            name: "bench/http".to_string(),
            address: "127.0.0.1".to_string(),
            port: 18_080,
            protocol: "LISTENER_PROTOCOL_HTTP".to_string(),
            ..Listener::default()
        });
    }
    if config.include_stream {
        listeners.push(Listener {
            name: "bench/tcp".to_string(),
            address: "127.0.0.1".to_string(),
            port: 19_090,
            protocol: "LISTENER_PROTOCOL_TCP".to_string(),
            ..Listener::default()
        });
    }

    listeners
}
