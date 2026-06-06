#[test]
fn publishes_http_apply_success_event() {
    let stats = RuntimeStats::shared();
    let events = stats.subscribe_apply_events();

    stats.observe_http_listener_reload_success("v1");

    let event = events.borrow().clone().expect("http apply event");
    assert_eq!(event.version, "v1");
    assert_eq!(event.plane, RuntimePlane::Http);
    assert_eq!(event.outcome, RuntimeApplyOutcome::Applied);
    assert!(event.message.is_empty());
}

#[test]
fn publishes_stream_apply_failure_event() {
    let stats = RuntimeStats::shared();
    let events = stats.subscribe_apply_events();

    stats.observe_stream_listener_reload_failure("v9", "default/tcp", "bind failed");

    let event = events.borrow().clone().expect("stream apply event");
    assert_eq!(event.version, "v9");
    assert_eq!(event.plane, RuntimePlane::Stream);
    assert_eq!(event.outcome, RuntimeApplyOutcome::Rejected);
    assert_eq!(event.message, "bind failed");
}
