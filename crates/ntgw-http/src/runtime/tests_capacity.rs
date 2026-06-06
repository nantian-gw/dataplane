use super::capacity::{
    effective_http_capacity, effective_http_capacity_with_parallelism, server_conf_for_capacity,
};
use super::HttpCapacityOptions;

#[test]
fn default_capacity_uses_high_concurrency_baseline() {
    let capacity = effective_http_capacity_with_parallelism(&HttpCapacityOptions::default(), 1);

    assert_eq!(capacity.worker_threads, Some(2));
    assert_eq!(capacity.accept_concurrency, Some(1));
    assert_eq!(capacity.upstream_keepalive_pool_size, Some(2048));
    assert_eq!(capacity.reuse_port, Some(true));

    let conf = server_conf_for_capacity(&capacity);
    assert_eq!(conf.threads, 2);
    assert_eq!(conf.listener_tasks_per_fd, 1);
    assert_eq!(conf.upstream_keepalive_pool_size, 2048);
}

#[test]
fn high_concurrency_capacity_sets_expected_baseline() {
    let options = HttpCapacityOptions::default();

    let capacity = effective_http_capacity_with_parallelism(&options, 1);
    assert_eq!(capacity.worker_threads, Some(2));
    assert_eq!(capacity.accept_concurrency, Some(1));
    assert_eq!(capacity.upstream_keepalive_pool_size, Some(2048));
    assert_eq!(capacity.reuse_port, Some(true));

    let conf = server_conf_for_capacity(&capacity);
    assert_eq!(conf.threads, 2);
    assert_eq!(conf.listener_tasks_per_fd, 1);
    assert_eq!(conf.upstream_keepalive_pool_size, 2048);
}

#[test]
fn high_concurrency_capacity_scales_accept_and_pool_with_available_parallelism() {
    let options = HttpCapacityOptions::default();

    let capacity = effective_http_capacity_with_parallelism(&options, 8);
    assert_eq!(capacity.worker_threads, Some(8));
    assert_eq!(capacity.accept_concurrency, Some(8));
    assert_eq!(capacity.upstream_keepalive_pool_size, Some(8192));
    assert_eq!(capacity.reuse_port, Some(true));

    let conf = server_conf_for_capacity(&capacity);
    assert_eq!(conf.threads, 8);
    assert_eq!(conf.listener_tasks_per_fd, 8);
    assert_eq!(conf.upstream_keepalive_pool_size, 8192);
}

#[test]
fn high_concurrency_capacity_caps_large_parallelism() {
    let options = HttpCapacityOptions::default();

    let capacity = effective_http_capacity_with_parallelism(&options, 128);
    assert_eq!(capacity.worker_threads, Some(32));
    assert_eq!(capacity.accept_concurrency, Some(16));
    assert_eq!(capacity.upstream_keepalive_pool_size, Some(32_768));
    assert_eq!(capacity.reuse_port, Some(true));
}

#[test]
fn explicit_capacity_overrides_default_baseline() {
    let options = HttpCapacityOptions {
        worker_threads: 4,
        accept_concurrency: 3,
        upstream_keepalive_pool_size: 1024,
        reuse_port: Some(false),
    };

    let capacity = effective_http_capacity(&options);
    assert_eq!(capacity.worker_threads, Some(4));
    assert_eq!(capacity.accept_concurrency, Some(3));
    assert_eq!(capacity.upstream_keepalive_pool_size, Some(1024));
    assert_eq!(capacity.reuse_port, Some(false));

    let conf = server_conf_for_capacity(&capacity);
    assert_eq!(conf.threads, 4);
    assert_eq!(conf.listener_tasks_per_fd, 3);
    assert_eq!(conf.upstream_keepalive_pool_size, 1024);
}

#[test]
fn runtime_server_conf_uses_runtime_capacity_options() {
    let runtime = RuntimeOptions {
        capacity: HttpCapacityOptions {
            worker_threads: 5,
            accept_concurrency: 4,
            upstream_keepalive_pool_size: 2048,
            reuse_port: Some(false),
        },
        ..RuntimeOptions::default()
    };

    let conf = server_conf_for_runtime(&runtime);
    assert_eq!(conf.threads, 5);
    assert_eq!(conf.listener_tasks_per_fd, 4);
    assert_eq!(conf.upstream_keepalive_pool_size, 2048);
}
