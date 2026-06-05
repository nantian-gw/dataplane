use std::fmt::Write as _;

pub(super) fn append_gauge(out: &mut String, name: &str, help: &str, value: u64) {
    let _ = writeln!(out, "# HELP {name} {help}");
    let _ = writeln!(out, "# TYPE {name} gauge");
    let _ = writeln!(out, "{name} {value}");
}

pub(super) fn append_gauge_f64(out: &mut String, name: &str, help: &str, value: f64) {
    let _ = writeln!(out, "# HELP {name} {help}");
    let _ = writeln!(out, "# TYPE {name} gauge");
    let _ = writeln!(out, "{name} {value}");
}

pub(super) fn append_counter_f64(out: &mut String, name: &str, help: &str, value: f64) {
    let _ = writeln!(out, "# HELP {name} {help}");
    let _ = writeln!(out, "# TYPE {name} counter");
    let _ = writeln!(out, "{name} {value}");
}

pub(super) fn append_counter(out: &mut String, name: &str, help: &str, value: u64) {
    let _ = writeln!(out, "# HELP {name} {help}");
    let _ = writeln!(out, "# TYPE {name} counter");
    let _ = writeln!(out, "{name} {value}");
}

pub(super) fn append_histogram<'a, I>(
    out: &mut String,
    name: &str,
    help: &str,
    buckets: I,
    sum: u64,
    count: u64,
) where
    I: IntoIterator<Item = (&'a str, u64)>,
{
    let _ = writeln!(out, "# HELP {name} {help}");
    let _ = writeln!(out, "# TYPE {name} histogram");
    for (le, value) in buckets {
        let _ = writeln!(
            out,
            "{name}_bucket{{le=\"{}\"}} {value}",
            prometheus_label(le)
        );
    }
    let _ = writeln!(out, "{name}_sum {sum}");
    let _ = writeln!(out, "{name}_count {count}");
}

pub(super) fn append_labeled_gauge_map(
    out: &mut String,
    name: &str,
    help: &str,
    label_name: &str,
    values: &std::collections::BTreeMap<String, u64>,
    known_labels: &[impl AsRef<str>],
) {
    append_labeled_map(out, name, help, "gauge", label_name, values, known_labels);
}

pub(super) fn append_labeled_counter_values(
    out: &mut String,
    name: &str,
    help: &str,
    label_name: &str,
    values: &[(&str, u64)],
) {
    let _ = writeln!(out, "# HELP {name} {help}");
    let _ = writeln!(out, "# TYPE {name} counter");
    for (label_value, value) in values {
        let _ = writeln!(
            out,
            "{name}{{{label_name}=\"{}\"}} {value}",
            prometheus_label(label_value)
        );
    }
}

pub(super) fn append_labeled_counter_map(
    out: &mut String,
    name: &str,
    help: &str,
    label_name: &str,
    values: &std::collections::BTreeMap<String, u64>,
    known_labels: &[impl AsRef<str>],
) {
    append_labeled_map(out, name, help, "counter", label_name, values, known_labels);
}

fn append_labeled_map<T: AsRef<str>>(
    out: &mut String,
    name: &str,
    help: &str,
    metric_type: &str,
    label_name: &str,
    values: &std::collections::BTreeMap<String, u64>,
    known_labels: &[T],
) {
    let labels = known_labels
        .iter()
        .map(AsRef::as_ref)
        .chain(values.keys().map(String::as_str))
        .collect::<std::collections::BTreeSet<_>>();
    if labels.is_empty() {
        return;
    }

    let _ = writeln!(out, "# HELP {name} {help}");
    let _ = writeln!(out, "# TYPE {name} {metric_type}");
    for label_value in labels {
        let value = values.get(label_value).copied().unwrap_or_default();
        let _ = writeln!(
            out,
            "{name}{{{label_name}=\"{}\"}} {value}",
            prometheus_label(label_value)
        );
    }
}

pub(super) fn prometheus_label(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}
