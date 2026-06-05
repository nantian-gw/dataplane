#[tokio::test]
async fn snapshot_signal_wakes_async_and_blocking_waiters() {
    let signal = SnapshotSignal::shared();
    let barrier = Arc::new(Barrier::new(2));
    let mut subscription = signal.subscribe();
    let blocking_signal = signal.clone();
    let blocking_barrier = barrier.clone();

    let waiter = std::thread::spawn(move || {
        blocking_barrier.wait();
        blocking_signal.wait_timeout(0, Duration::from_secs(5))
    });

    barrier.wait();
    let generation = signal.notify_changed();

    subscription
        .changed()
        .await
        .expect("watch receiver should observe the snapshot change");

    assert_eq!(generation, 1);
    assert_eq!(*subscription.borrow(), 1);
    assert_eq!(waiter.join().expect("blocking waiter should finish"), 1);
}

#[test]
fn snapshot_signal_timeout_returns_without_a_change() {
    let signal = SnapshotSignal::shared();
    let observed = signal.wait_timeout(0, Duration::from_millis(5));
    assert_eq!(observed, 0);
}
