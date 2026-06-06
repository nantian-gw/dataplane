mod attention;
mod convergence;
mod counts;
mod current;
mod recovery;
mod serving;

use self::{
    attention::append_listener_attention_metrics, convergence::append_listener_convergence_metrics,
    counts::collect_listener_metric_counts, current::append_listener_current_metrics,
    recovery::append_listener_recovery_metrics, serving::append_listener_serving_metrics,
};
use super::context::MetricsContext;

pub(super) fn append_listener_metrics(out: &mut String, ctx: &MetricsContext) {
    let counts = collect_listener_metric_counts(ctx);

    append_listener_current_metrics(out, &counts);
    append_listener_convergence_metrics(out, ctx, &counts);
    append_listener_serving_metrics(out, &counts);
    append_listener_recovery_metrics(out, ctx, &counts);
    append_listener_attention_metrics(out, &counts);
}
