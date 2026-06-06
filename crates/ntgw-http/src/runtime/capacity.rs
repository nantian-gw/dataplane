use pingora::server::configuration::ServerConf;

const HIGH_CONCURRENCY_MIN_WORKER_THREADS: usize = 2;
const HIGH_CONCURRENCY_MAX_WORKER_THREADS: usize = 32;
const HIGH_CONCURRENCY_MAX_ACCEPT_CONCURRENCY: usize = 16;
const HIGH_CONCURRENCY_UPSTREAM_KEEPALIVE_POOL_SIZE_PER_WORKER: usize = 1024;
const HIGH_CONCURRENCY_MAX_UPSTREAM_KEEPALIVE_POOL_SIZE: usize = 32_768;
const CGROUP_V2_CPU_MAX_PATH: &str = "/sys/fs/cgroup/cpu.max";
const CGROUP_V1_CPU_QUOTA_PATH: &str = "/sys/fs/cgroup/cpu/cpu.cfs_quota_us";
const CGROUP_V1_CPU_PERIOD_PATH: &str = "/sys/fs/cgroup/cpu/cpu.cfs_period_us";

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct HttpCapacityOptions {
    pub worker_threads: usize,
    pub accept_concurrency: usize,
    pub upstream_keepalive_pool_size: usize,
    pub reuse_port: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EffectiveHttpCapacity {
    pub worker_threads: Option<usize>,
    pub accept_concurrency: Option<usize>,
    pub upstream_keepalive_pool_size: Option<usize>,
    pub reuse_port: Option<bool>,
}

pub fn effective_http_capacity(options: &HttpCapacityOptions) -> EffectiveHttpCapacity {
    effective_http_capacity_with_parallelism(options, available_http_parallelism())
}

pub fn effective_http_capacity_with_parallelism(
    options: &HttpCapacityOptions,
    parallelism: usize,
) -> EffectiveHttpCapacity {
    let (worker_threads, accept_concurrency, upstream_keepalive_pool_size, reuse_port) =
        high_concurrency_capacity(parallelism);

    EffectiveHttpCapacity {
        worker_threads: positive_capacity(options.worker_threads, worker_threads),
        accept_concurrency: positive_capacity(options.accept_concurrency, accept_concurrency),
        upstream_keepalive_pool_size: positive_capacity(
            options.upstream_keepalive_pool_size,
            upstream_keepalive_pool_size,
        ),
        reuse_port: options.reuse_port.or(reuse_port),
    }
}

pub fn server_conf_for_capacity(capacity: &EffectiveHttpCapacity) -> ServerConf {
    let mut conf = ServerConf::default();
    if let Some(worker_threads) = capacity.worker_threads {
        conf.threads = worker_threads;
    }
    if let Some(accept_concurrency) = capacity.accept_concurrency {
        conf.listener_tasks_per_fd = accept_concurrency;
    }
    if let Some(upstream_keepalive_pool_size) = capacity.upstream_keepalive_pool_size {
        conf.upstream_keepalive_pool_size = upstream_keepalive_pool_size;
    }
    conf
}

fn high_concurrency_capacity(parallelism: usize) -> (usize, usize, usize, Option<bool>) {
    let worker_threads = parallelism.clamp(
        HIGH_CONCURRENCY_MIN_WORKER_THREADS,
        HIGH_CONCURRENCY_MAX_WORKER_THREADS,
    );
    let accept_concurrency = if worker_threads <= HIGH_CONCURRENCY_MIN_WORKER_THREADS {
        1
    } else {
        worker_threads.min(HIGH_CONCURRENCY_MAX_ACCEPT_CONCURRENCY)
    };
    let upstream_keepalive_pool_size = worker_threads
        .saturating_mul(HIGH_CONCURRENCY_UPSTREAM_KEEPALIVE_POOL_SIZE_PER_WORKER)
        .clamp(
            HIGH_CONCURRENCY_UPSTREAM_KEEPALIVE_POOL_SIZE_PER_WORKER,
            HIGH_CONCURRENCY_MAX_UPSTREAM_KEEPALIVE_POOL_SIZE,
        );

    (
        worker_threads,
        accept_concurrency,
        upstream_keepalive_pool_size,
        Some(true),
    )
}

fn positive_capacity(override_value: usize, profile_value: usize) -> Option<usize> {
    let value = if override_value > 0 {
        override_value
    } else {
        profile_value
    };
    (value > 0).then_some(value)
}

fn available_http_parallelism() -> usize {
    cgroup_cpu_quota_parallelism()
        .or_else(|| {
            std::thread::available_parallelism()
                .ok()
                .map(|parallelism| parallelism.get())
        })
        .unwrap_or(1)
}

fn cgroup_cpu_quota_parallelism() -> Option<usize> {
    std::fs::read_to_string(CGROUP_V2_CPU_MAX_PATH)
        .ok()
        .as_deref()
        .and_then(parse_cgroup_v2_cpu_max)
        .or_else(cgroup_v1_cpu_quota_parallelism)
}

fn cgroup_v1_cpu_quota_parallelism() -> Option<usize> {
    let quota = std::fs::read_to_string(CGROUP_V1_CPU_QUOTA_PATH)
        .ok()?
        .trim()
        .parse::<i64>()
        .ok()?;
    let period = std::fs::read_to_string(CGROUP_V1_CPU_PERIOD_PATH)
        .ok()?
        .trim()
        .parse::<i64>()
        .ok()?;
    parse_cpu_quota_period(quota, period)
}

fn parse_cgroup_v2_cpu_max(raw: &str) -> Option<usize> {
    let mut fields = raw.split_whitespace();
    let quota = fields.next()?;
    if quota == "max" {
        return None;
    }
    let quota = quota.parse::<i64>().ok()?;
    let period = fields.next()?.parse::<i64>().ok()?;
    parse_cpu_quota_period(quota, period)
}

fn parse_cpu_quota_period(quota: i64, period: i64) -> Option<usize> {
    if quota <= 0 || period <= 0 {
        return None;
    }
    Some((quota as usize).div_ceil(period as usize).max(1))
}
