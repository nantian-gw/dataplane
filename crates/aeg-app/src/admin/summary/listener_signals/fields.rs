use serde_json::{json, Map, Value};

use super::helpers::{NamedListeners, PlaneNamedListeners};

pub(super) fn insert_named_value(map: &mut Map<String, Value>, key: &str, value: usize) {
    map.insert(key.to_string(), json!(value));
}

pub(super) fn insert_name_list(map: &mut Map<String, Value>, key: &str, names: &[String]) {
    map.insert(key.to_string(), json!(names));
}

pub(super) fn insert_runtime_id_list(
    map: &mut Map<String, Value>,
    key: &str,
    runtime_ids: &[String],
) {
    map.insert(key.to_string(), json!(runtime_ids));
}

pub(super) fn insert_named_fields(
    map: &mut Map<String, Value>,
    prefix: &str,
    listeners: &NamedListeners,
) {
    insert_named_value(map, &format!("{prefix}Count"), listeners.count);
    insert_name_list(map, &format!("{prefix}Names"), &listeners.names);
    insert_runtime_id_list(map, &format!("{prefix}RuntimeIds"), &listeners.runtime_ids);
}

pub(super) fn insert_plane_suffix_fields(
    map: &mut Map<String, Value>,
    prefix: &str,
    listeners: &PlaneNamedListeners,
) {
    for (suffix, named) in [
        ("Http", &listeners.http),
        ("Tls", &listeners.tls),
        ("Stream", &listeners.stream),
        ("None", &listeners.none),
    ] {
        insert_named_value(map, &format!("{prefix}{suffix}Count"), named.count);
        insert_name_list(map, &format!("{prefix}{suffix}Names"), &named.names);
        insert_runtime_id_list(
            map,
            &format!("{prefix}{suffix}RuntimeIds"),
            &named.runtime_ids,
        );
    }
}

pub(super) fn insert_plane_named_fields(
    map: &mut Map<String, Value>,
    prefix: &str,
    listeners: &PlaneNamedListeners,
) {
    insert_named_fields(map, prefix, &listeners.total);
    insert_plane_suffix_fields(map, prefix, listeners);
}

#[allow(clippy::too_many_arguments)]
pub(super) fn insert_overview_fields(
    map: &mut Map<String, Value>,
    prefix: &str,
    severity: &str,
    severity_level: u64,
    primary_signal: &str,
    recommended_filter: &str,
    recommended_path: &str,
    recommended_reason: &str,
    recommended_count: usize,
    overview: &Value,
) {
    map.insert(format!("{prefix}Severity"), json!(severity));
    map.insert(format!("{prefix}SeverityLevel"), json!(severity_level));
    map.insert(format!("{prefix}PrimarySignal"), json!(primary_signal));
    map.insert(
        format!("{prefix}RecommendedFilter"),
        json!(recommended_filter),
    );
    map.insert(format!("{prefix}RecommendedPath"), json!(recommended_path));
    map.insert(
        format!("{prefix}RecommendedReason"),
        json!(recommended_reason),
    );
    map.insert(
        format!("{prefix}RecommendedCount"),
        json!(recommended_count),
    );
    map.insert(format!("{prefix}Overview"), overview.clone());
}
