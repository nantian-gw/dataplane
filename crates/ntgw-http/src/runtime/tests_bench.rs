#[test]
fn tls_rotation_fixture_reuses_assets_and_cleans_stale_files() {
    let fixture = super::bench::TlsRotationFixture::build(super::bench::TlsRotationBenchConfig {
        listeners: 8,
        ca_bundle_variants: 2,
    });
    let asset_dir = std::env::temp_dir().join(format!(
        "ntgw-http-bench-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock should be monotonic enough for test dirs")
            .as_nanos()
    ));

    let initial = fixture
        .materialize_initial(&asset_dir)
        .expect("materialize initial tls assets");
    let rotated = fixture.rotate(&asset_dir).expect("rotate tls assets");
    let remaining_files = fixture
        .cleanup_rotated(&asset_dir)
        .expect("cleanup rotated tls assets");
    let asset_names = fs::read_dir(&asset_dir)
        .expect("read asset dir")
        .map(|entry| {
            entry
                .expect("asset dir entry")
                .file_name()
                .to_string_lossy()
                .into_owned()
        })
        .collect::<Vec<_>>();

    assert_eq!(initial.listener_count, 8);
    assert!(initial.reused_assets > 0);
    assert_eq!(rotated.unique_asset_prefixes, 2);
    assert_eq!(remaining_files, rotated.file_count);
    assert!(asset_names
        .iter()
        .all(|name| !name.starts_with(".ntgw-tls-asset-tmp-")));

    fs::remove_dir_all(&asset_dir).expect("cleanup asset dir");
}

#[test]
fn http_capacity_matrix_fixture_records_default_and_tuned_profiles() {
    let fixture = super::bench::HttpCapacityMatrixFixture::build(
        super::bench::HttpCapacityMatrixBenchConfig {
            parallelism_samples: [1, 2, 4, 8, 16, 32, 64, 128],
            tuned_worker_threads: 6,
            tuned_accept_concurrency: 5,
            tuned_upstream_keepalive_pool_size: 8192,
            tuned_reuse_port: false,
        },
    );

    let step = fixture.evaluate_once();

    assert_eq!(step.row_count, 16);
    assert_eq!(step.default_rows, 8);
    assert_eq!(step.tuned_rows, 8);
    assert_eq!(step.min_parallelism, 1);
    assert_eq!(step.max_parallelism, 128);

    let one_cpu_default = step
        .rows
        .iter()
        .find(|row| row.profile == "default" && row.parallelism == 1)
        .expect("1-cpu default capacity row");
    assert_eq!(one_cpu_default.effective_worker_threads, Some(2));
    assert_eq!(one_cpu_default.effective_accept_concurrency, Some(1));
    assert_eq!(
        one_cpu_default.effective_upstream_keepalive_pool_size,
        Some(2048)
    );
    assert_eq!(one_cpu_default.effective_reuse_port, Some(true));
    assert_eq!(
        one_cpu_default.server_threads,
        one_cpu_default.effective_worker_threads.unwrap()
    );

    let tuned = step
        .rows
        .iter()
        .find(|row| row.profile == "tuned" && row.parallelism == 128)
        .expect("tuned capacity row");
    assert_eq!(tuned.effective_worker_threads, Some(6));
    assert_eq!(tuned.effective_accept_concurrency, Some(5));
    assert_eq!(tuned.effective_upstream_keepalive_pool_size, Some(8192));
    assert_eq!(tuned.effective_reuse_port, Some(false));
    assert_eq!(tuned.server_listener_tasks_per_fd, 5);
}
