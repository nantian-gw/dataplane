use serde::{Deserialize, Serialize};

use super::capacity::{effective_http_capacity_with_parallelism, server_conf_for_capacity};
use super::*;

const BENCH_SERVER_CERT_PEM: &str = include_str!("../../../../testdata/backendtls/server-san.crt");
const BENCH_SERVER_KEY_PEM: &str = include_str!("../../../../testdata/backendtls/server-san.key");

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct TlsRotationBenchConfig {
    pub listeners: usize,
    pub ca_bundle_variants: usize,
}

impl Default for TlsRotationBenchConfig {
    fn default() -> Self {
        Self {
            listeners: 32,
            ca_bundle_variants: 4,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TlsRotationStep {
    pub listener_count: usize,
    pub unique_asset_prefixes: usize,
    pub reused_assets: u64,
    pub file_count: usize,
}

#[derive(Debug, Clone)]
pub struct TlsRotationFixture {
    initial: ListenerPlan,
    rotated: ListenerPlan,
}

impl TlsRotationFixture {
    pub fn build(config: TlsRotationBenchConfig) -> Self {
        let listeners = config.listeners.max(1);
        let variants = config.ca_bundle_variants.max(1);
        Self {
            initial: tls_rotation_plan(listeners, 1),
            rotated: tls_rotation_plan(listeners, variants),
        }
    }

    pub fn materialize_initial(&self, root: &Path) -> Result<TlsRotationStep> {
        materialize_step(&self.initial, root)
    }

    pub fn rotate(&self, root: &Path) -> Result<TlsRotationStep> {
        materialize_step(&self.rotated, root)
    }

    pub fn cleanup_rotated(&self, root: &Path) -> Result<usize> {
        let referenced = referenced_tls_asset_prefixes(&self.rotated);
        cleanup_unused_tls_assets_in_dir(root, &referenced)?;
        count_files(root)
    }
}

fn materialize_step(plan: &ListenerPlan, root: &Path) -> Result<TlsRotationStep> {
    let stats = materialize_tls_assets_in_dir(plan, root)?;
    Ok(TlsRotationStep {
        listener_count: plan.listeners.len(),
        unique_asset_prefixes: referenced_tls_asset_prefixes(plan).len(),
        reused_assets: stats.reused,
        file_count: count_files(root)?,
    })
}

fn tls_rotation_plan(listeners: usize, variants: usize) -> ListenerPlan {
    ListenerPlan {
        listeners: (0..listeners)
            .map(|index| PlannedListener {
                name: format!("default/gw/https-{index}"),
                bind: format!("127.0.0.1:{}", 14_443 + index),
                protocol: ListenerProtocol::Tls(TlsMaterial {
                    identities: vec![TlsIdentity {
                        secret_ref: "default/bench-cert".to_string(),
                        cert_pem: BENCH_SERVER_CERT_PEM.to_string(),
                        key_pem: BENCH_SERVER_KEY_PEM.to_string(),
                        match_names: Vec::new(),
                    }],
                    min_version: "1.2".to_string(),
                    max_version: "1.3".to_string(),
                    client_ca_bundle_pem: Some(bench_ca_bundle(index % variants)),
                    frontend_validation_mode: None,
                }),
            })
            .collect(),
    }
}

fn bench_ca_bundle(variant: usize) -> String {
    format!("CA-BUNDLE-{variant}\nINTERMEDIATE-{variant}")
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct HttpCapacityMatrixBenchConfig {
    pub parallelism_samples: [usize; 8],
    pub tuned_worker_threads: usize,
    pub tuned_accept_concurrency: usize,
    pub tuned_upstream_keepalive_pool_size: usize,
    pub tuned_reuse_port: bool,
}

impl Default for HttpCapacityMatrixBenchConfig {
    fn default() -> Self {
        Self {
            parallelism_samples: [1, 2, 4, 8, 16, 32, 64, 128],
            tuned_worker_threads: 4,
            tuned_accept_concurrency: 3,
            tuned_upstream_keepalive_pool_size: 4096,
            tuned_reuse_port: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HttpCapacityMatrixRow {
    pub profile: String,
    pub parallelism: usize,
    pub requested_worker_threads: usize,
    pub requested_accept_concurrency: usize,
    pub requested_upstream_keepalive_pool_size: usize,
    pub requested_reuse_port: Option<bool>,
    pub effective_worker_threads: Option<usize>,
    pub effective_accept_concurrency: Option<usize>,
    pub effective_upstream_keepalive_pool_size: Option<usize>,
    pub effective_reuse_port: Option<bool>,
    pub server_threads: usize,
    pub server_listener_tasks_per_fd: usize,
    pub server_upstream_keepalive_pool_size: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HttpCapacityMatrixStep {
    pub row_count: usize,
    pub min_parallelism: usize,
    pub max_parallelism: usize,
    pub default_rows: usize,
    pub tuned_rows: usize,
    pub rows: Vec<HttpCapacityMatrixRow>,
}

#[derive(Debug, Clone)]
pub struct HttpCapacityMatrixFixture {
    config: HttpCapacityMatrixBenchConfig,
}

impl HttpCapacityMatrixFixture {
    pub fn build(config: HttpCapacityMatrixBenchConfig) -> Self {
        Self { config }
    }

    pub fn evaluate_once(&self) -> HttpCapacityMatrixStep {
        let default_options = HttpCapacityOptions::default();
        let tuned_options = HttpCapacityOptions {
            worker_threads: self.config.tuned_worker_threads,
            accept_concurrency: self.config.tuned_accept_concurrency,
            upstream_keepalive_pool_size: self.config.tuned_upstream_keepalive_pool_size,
            reuse_port: Some(self.config.tuned_reuse_port),
        };

        let mut rows = Vec::with_capacity(self.config.parallelism_samples.len() * 2);
        for &parallelism in &self.config.parallelism_samples {
            rows.push(http_capacity_matrix_row(
                "default",
                parallelism,
                &default_options,
            ));
            rows.push(http_capacity_matrix_row(
                "tuned",
                parallelism,
                &tuned_options,
            ));
        }

        let row_count = rows.len();
        let min_parallelism = self
            .config
            .parallelism_samples
            .iter()
            .copied()
            .min()
            .unwrap_or_default();
        let max_parallelism = self
            .config
            .parallelism_samples
            .iter()
            .copied()
            .max()
            .unwrap_or_default();
        let default_rows = rows.iter().filter(|row| row.profile == "default").count();
        let tuned_rows = rows.iter().filter(|row| row.profile == "tuned").count();

        HttpCapacityMatrixStep {
            row_count,
            min_parallelism,
            max_parallelism,
            default_rows,
            tuned_rows,
            rows,
        }
    }
}

fn http_capacity_matrix_row(
    profile: &str,
    parallelism: usize,
    options: &HttpCapacityOptions,
) -> HttpCapacityMatrixRow {
    let effective = effective_http_capacity_with_parallelism(options, parallelism);
    let server_conf = server_conf_for_capacity(&effective);
    HttpCapacityMatrixRow {
        profile: profile.to_string(),
        parallelism,
        requested_worker_threads: options.worker_threads,
        requested_accept_concurrency: options.accept_concurrency,
        requested_upstream_keepalive_pool_size: options.upstream_keepalive_pool_size,
        requested_reuse_port: options.reuse_port,
        effective_worker_threads: effective.worker_threads,
        effective_accept_concurrency: effective.accept_concurrency,
        effective_upstream_keepalive_pool_size: effective.upstream_keepalive_pool_size,
        effective_reuse_port: effective.reuse_port,
        server_threads: server_conf.threads,
        server_listener_tasks_per_fd: server_conf.listener_tasks_per_fd,
        server_upstream_keepalive_pool_size: server_conf.upstream_keepalive_pool_size,
    }
}

fn count_files(root: &Path) -> Result<usize> {
    if !root.is_dir() {
        return Ok(0);
    }

    Ok(fs::read_dir(root)?
        .filter_map(|entry| entry.ok())
        .filter(|entry| entry.path().is_file())
        .count())
}
