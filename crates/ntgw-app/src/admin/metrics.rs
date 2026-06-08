mod admin;
mod context;
mod listeners;
mod node_info;
mod overview;
mod prometheus;
mod traffic;

use super::AppState;
use admin::append_admin_request_metrics;
use context::MetricsContext;
use listeners::append_listener_metrics;
use node_info::append_node_info_metrics;
use overview::append_overview_metrics;

pub(super) fn render_metrics(state: &AppState) -> String {
    let ctx = MetricsContext::from_state(state);
    let mut out = String::new();

    append_overview_metrics(&mut out, &ctx);
    append_listener_metrics(&mut out, &ctx);
    append_node_info_metrics(&mut out, &ctx);
    append_admin_request_metrics(&mut out, &ctx);

    out
}
