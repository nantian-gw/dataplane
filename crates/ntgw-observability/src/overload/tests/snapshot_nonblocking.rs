use super::*;

#[test]
fn http_global_admission_does_not_block_on_listener_snapshot_reads() {
    let stats = OverloadStats::shared();
    let controller = HttpAdmissionController::new(
        HttpAdmissionOptions {
            global_inflight_limit: 1,
            ..HttpAdmissionOptions::default()
        },
        stats.clone(),
    );
    let held_read = hold_http_listener_inflight_snapshot_read(&stats);
    let (tx, rx) = mpsc::channel();

    thread::spawn(move || {
        let permit = controller
            .try_acquire_listener("default/gw/http")
            .expect("global admission should succeed");
        tx.send(permit.active_budget_count())
            .expect("send admission result");
    });

    assert_eq!(rx.recv_timeout(Duration::from_millis(100)), Ok(1));
    drop(held_read);
}
