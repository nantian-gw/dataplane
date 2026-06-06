use super::ShutdownCoordinator;

#[test]
fn shutdown_coordinator_only_notifies_once() {
    let coordinator = ShutdownCoordinator::new();

    assert!(coordinator.request_shutdown());
    assert!(!coordinator.request_shutdown());
    assert!(*coordinator.subscribe().borrow());
}
