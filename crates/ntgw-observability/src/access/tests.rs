use std::{
    borrow::Cow,
    collections::BTreeMap,
    fs,
    sync::mpsc,
    time::{SystemTime, UNIX_EPOCH},
};

use super::writer::AccessLogCommand;
use super::{
    AccessLogMode, AccessLogOptions, AccessLogRecord, AccessLogSampleKey,
    access_log_enabled_for_route, flush_access_log, render_access_log, resolve_access_log_options,
    resolve_access_log_write_options, should_emit_sample, write_access_log,
};
use super::{
    template::{
        AccessLogPlaceholder, AccessLogTemplatePart, parse_access_log_template,
        render_access_log_template,
    },
    writer::{AccessLogWriter, queue_access_log_line, spawn_access_log_writer_for_test, LogTarget},
};

include!("tests/rendering.rs");
include!("tests/nginx_style.rs");
include!("tests/route_overrides.rs");
include!("tests/sampling.rs");
include!("tests/writer.rs");

fn temp_log_path(prefix: &str) -> std::path::PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time")
        .as_nanos();
    std::env::temp_dir().join(format!("ntgw-observability-{prefix}-{unique}.log"))
}
