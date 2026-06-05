use super::super::super::types::ListenerRuntimeStatus;

#[derive(Default, Clone)]
pub(super) struct NamedListeners {
    pub(super) count: usize,
    pub(super) names: Vec<String>,
    pub(super) runtime_ids: Vec<String>,
}

#[derive(Default, Clone)]
pub(super) struct PlaneNamedListeners {
    pub(super) total: NamedListeners,
    pub(super) http: NamedListeners,
    pub(super) tls: NamedListeners,
    pub(super) stream: NamedListeners,
    pub(super) none: NamedListeners,
}

pub(super) fn collect_named<F>(
    listener_runtime_statuses: &[ListenerRuntimeStatus],
    include_empty_names: bool,
    predicate: F,
) -> NamedListeners
where
    F: Fn(&ListenerRuntimeStatus) -> bool,
{
    let mut listeners = NamedListeners::default();

    for status in listener_runtime_statuses {
        if predicate(status) {
            push_named_listener(&mut listeners, status, include_empty_names);
        }
    }

    listeners
}

pub(super) fn collect_plane_named<F>(
    listener_runtime_statuses: &[ListenerRuntimeStatus],
    include_empty_names: bool,
    predicate: F,
) -> PlaneNamedListeners
where
    F: Fn(&ListenerRuntimeStatus) -> bool,
{
    let mut listeners = PlaneNamedListeners::default();

    for status in listener_runtime_statuses {
        if !predicate(status) {
            continue;
        }

        push_named_listener(&mut listeners.total, status, include_empty_names);
        match status.runtime_plane.as_str() {
            "http" => push_named_listener(&mut listeners.http, status, include_empty_names),
            "tls" => push_named_listener(&mut listeners.tls, status, include_empty_names),
            "stream" => push_named_listener(&mut listeners.stream, status, include_empty_names),
            _ => push_named_listener(&mut listeners.none, status, include_empty_names),
        }
    }

    listeners
}

fn push_named_listener(
    listeners: &mut NamedListeners,
    status: &ListenerRuntimeStatus,
    include_empty_names: bool,
) {
    listeners.count += 1;
    if include_empty_names || !status.listener.name.is_empty() {
        listeners.names.push(status.listener.name.clone());
        if let Some(runtime_id) = status.runtime_id.as_ref() {
            listeners.runtime_ids.push(runtime_id.clone());
        }
    }
}

pub(super) fn count_matching<F>(
    listener_runtime_statuses: &[ListenerRuntimeStatus],
    predicate: F,
) -> usize
where
    F: Fn(&ListenerRuntimeStatus) -> bool,
{
    listener_runtime_statuses
        .iter()
        .filter(|status| predicate(status))
        .count()
}

pub(super) fn count_field<F>(
    listener_runtime_statuses: &[ListenerRuntimeStatus],
    field: F,
    expected: &str,
) -> usize
where
    F: Fn(&ListenerRuntimeStatus) -> &str,
{
    listener_runtime_statuses
        .iter()
        .filter(|status| field(status) == expected)
        .count()
}

pub(super) fn has_attention_reason(status: &ListenerRuntimeStatus, reason: &str) -> bool {
    status
        .listener_attention_reasons
        .iter()
        .any(|candidate| candidate == reason)
}

pub(super) fn filter_non_empty_names(names: &[String]) -> Vec<String> {
    names
        .iter()
        .filter(|name| !name.is_empty())
        .cloned()
        .collect()
}
