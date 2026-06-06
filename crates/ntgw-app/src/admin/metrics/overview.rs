mod inventory;
mod protection;
mod runtime;
mod traffic;
mod xds;

use self::{
    inventory::append_inventory_metrics, protection::append_protection_metrics,
    runtime::append_runtime_metrics, traffic::append_traffic_metrics, xds::append_xds_metrics,
};
use super::context::MetricsContext;

pub(super) fn append_overview_metrics(out: &mut String, ctx: &MetricsContext) {
    append_inventory_metrics(out, ctx);
    append_protection_metrics(out, ctx);
    append_traffic_metrics(out, ctx);
    append_xds_metrics(out, ctx);
    append_runtime_metrics(out, ctx);
}
