use std::fmt::Write as _;

use super::{context::MetricsContext, prometheus::prometheus_label};
use aeg_observability::AdminRequestMetricSeries;

pub(super) fn append_admin_request_metrics(out: &mut String, ctx: &MetricsContext) {
    if ctx.admin_requests.series.is_empty() {
        return;
    }

    out.push_str(
        "# HELP aether_gateway_dataplane_admin_requests_total Total dataplane admin HTTP requests partitioned by method, normalized route, and status class.\n",
    );
    out.push_str("# TYPE aether_gateway_dataplane_admin_requests_total counter\n");
    for series in &ctx.admin_requests.series {
        append_admin_request_metric_labels(
            out,
            "aether_gateway_dataplane_admin_requests_total",
            series,
        );
        let _ = writeln!(out, "}} {}", series.total_requests);
    }

    out.push_str(
        "# HELP aether_gateway_dataplane_admin_request_duration_seconds Dataplane admin HTTP request duration partitioned by method, normalized route, and status class.\n",
    );
    out.push_str("# TYPE aether_gateway_dataplane_admin_request_duration_seconds histogram\n");
    for series in &ctx.admin_requests.series {
        for bucket in &series.duration_seconds_buckets {
            append_admin_request_metric_labels(
                out,
                "aether_gateway_dataplane_admin_request_duration_seconds_bucket",
                series,
            );
            let _ = writeln!(
                out,
                ",le=\"{}\"}} {}",
                prometheus_label(bucket.le.as_str()),
                bucket.count
            );
        }
        append_admin_request_metric_labels(
            out,
            "aether_gateway_dataplane_admin_request_duration_seconds_sum",
            series,
        );
        let _ = writeln!(out, "}} {}", series.duration_seconds_sum);
        append_admin_request_metric_labels(
            out,
            "aether_gateway_dataplane_admin_request_duration_seconds_count",
            series,
        );
        let _ = writeln!(out, "}} {}", series.duration_seconds_count);
    }
}

fn append_admin_request_metric_labels(
    out: &mut String,
    metric: &str,
    series: &AdminRequestMetricSeries,
) {
    let _ = write!(
        out,
        "{}{{method=\"{}\",route=\"{}\",status_class=\"{}\"",
        metric,
        prometheus_label(series.method.as_str()),
        prometheus_label(series.route.as_str()),
        prometheus_label(series.status_class.as_str()),
    );
}
