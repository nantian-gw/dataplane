use super::*;

#[test]
fn reconnect_backoff_grows_and_caps() {
    let mut backoff = ReconnectBackoff::new(&TransportOptions {
        initial_reconnect_backoff: Duration::from_millis(200),
        max_reconnect_backoff: Duration::from_millis(800),
        ..TransportOptions::default()
    });

    let first = backoff.next_delay();
    let second = backoff.next_delay();
    let third = backoff.next_delay();
    let fourth = backoff.next_delay();

    assert!(first >= Duration::from_millis(100));
    assert!(first <= Duration::from_millis(200));
    assert!(second >= first);
    assert!(third >= second);
    assert!(fourth <= Duration::from_millis(800));
    backoff.reset();
    let reset = backoff.next_delay();
    assert!(reset <= Duration::from_millis(200));
}

#[test]
fn established_stream_failure_resets_reconnect_backoff_before_retry() {
    let mut backoff = ReconnectBackoff::new(&TransportOptions {
        initial_reconnect_backoff: Duration::from_millis(200),
        max_reconnect_backoff: Duration::from_millis(800),
        ..TransportOptions::default()
    });

    let first = retry_delay_after_stream_failure(&mut backoff, false);
    let second = retry_delay_after_stream_failure(&mut backoff, false);
    assert!(second >= first);

    let after_established_failure = retry_delay_after_stream_failure(&mut backoff, true);
    assert!(
        after_established_failure <= Duration::from_millis(200),
        "expected established stream failure to reset backoff, got {after_established_failure:?}"
    );
}
