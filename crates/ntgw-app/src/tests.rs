use tokio::time::{Duration, advance};

use super::{
    initiate_runtime_shutdown,
    supervisor::{ShutdownCause, ShutdownCoordinator},
};
use ntgw_observability::RuntimeStats;

#[test]
fn dataplane_binary_reports_selected_allocator() {
    let expected = if cfg!(feature = "allocator-mimalloc") {
        "mimalloc"
    } else if cfg!(feature = "allocator-jemalloc") {
        "jemalloc"
    } else {
        "system"
    };

    assert_eq!(ntgw_allocator::selected_allocator(), expected);
}

#[tokio::test(start_paused = true)]
async fn graceful_shutdown_waits_for_configured_drain_before_notifying_runtimes() {
    let runtime_stats = RuntimeStats::shared();
    runtime_stats.observe_supervisor_started();
    let supervisor = ShutdownCoordinator::new();
    let mut shutdown = supervisor.subscribe();

    let shutdown_task = tokio::spawn(initiate_runtime_shutdown(
        runtime_stats.clone(),
        supervisor.clone(),
        ShutdownCause::graceful("signal: sigterm"),
        Duration::from_millis(5_000),
    ));

    tokio::task::yield_now().await;

    let snapshot = runtime_stats.snapshot();
    assert!(snapshot.supervisor_shutdown_requested);
    assert_eq!(snapshot.supervisor_last_shutdown_reason, "signal: sigterm");
    assert!(!*shutdown.borrow());

    advance(Duration::from_millis(4_999)).await;
    assert!(!*shutdown.borrow());

    advance(Duration::from_millis(1)).await;
    shutdown
        .changed()
        .await
        .expect("shutdown signal should be delivered after drain");
    assert!(*shutdown.borrow());

    shutdown_task
        .await
        .expect("shutdown helper should complete cleanly");
}

#[tokio::test(start_paused = true)]
async fn fatal_shutdown_notifies_runtimes_without_waiting_for_drain() {
    let runtime_stats = RuntimeStats::shared();
    runtime_stats.observe_supervisor_started();
    let supervisor = ShutdownCoordinator::new();
    let mut shutdown = supervisor.subscribe();

    let shutdown_task = tokio::spawn(initiate_runtime_shutdown(
        runtime_stats.clone(),
        supervisor.clone(),
        ShutdownCause::fatal("http runtime exited"),
        Duration::from_secs(30),
    ));

    shutdown
        .changed()
        .await
        .expect("fatal shutdown should notify runtimes immediately");
    assert!(*shutdown.borrow());

    let snapshot = runtime_stats.snapshot();
    assert!(snapshot.supervisor_shutdown_requested);
    assert_eq!(
        snapshot.supervisor_last_shutdown_reason,
        "http runtime exited"
    );

    shutdown_task
        .await
        .expect("shutdown helper should complete cleanly");
}
