use super::*;
use std::time::{SystemTime, UNIX_EPOCH};

pub(super) fn epoch_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|value| value.as_secs())
        .unwrap_or_default()
}

pub(super) fn record_listener_event(
    state: &mut RuntimeListenerProgress,
    event: RuntimeListenerEvent,
) {
    state.recent_events.insert(0, event);
    if state.recent_events.len() > LISTENER_EVENT_HISTORY_LIMIT {
        state.recent_events.truncate(LISTENER_EVENT_HISTORY_LIMIT);
    }
}
