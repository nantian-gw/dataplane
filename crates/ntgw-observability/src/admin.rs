use std::{collections::BTreeMap, sync::Arc, time::Duration};

use parking_lot::RwLock;

pub type SharedAdminRequestStats = Arc<AdminRequestStats>;

const ADMIN_REQUEST_DURATION_SECONDS_BUCKETS: [(f64, &str); 12] = [
    (0.001, "0.001"),
    (0.005, "0.005"),
    (0.01, "0.01"),
    (0.025, "0.025"),
    (0.05, "0.05"),
    (0.1, "0.1"),
    (0.25, "0.25"),
    (0.5, "0.5"),
    (1.0, "1"),
    (2.5, "2.5"),
    (5.0, "5"),
    (10.0, "10"),
];
const ADMIN_REQUEST_DURATION_SECONDS_BUCKET_COUNT: usize =
    ADMIN_REQUEST_DURATION_SECONDS_BUCKETS.len() + 1;

#[derive(Debug, Clone, Default, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AdminRequestStatsSnapshot {
    pub series: Vec<AdminRequestMetricSeries>,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AdminRequestMetricSeries {
    pub method: String,
    pub route: String,
    pub status_class: String,
    pub total_requests: u64,
    pub duration_seconds_sum: f64,
    pub duration_seconds_count: u64,
    pub duration_seconds_buckets: Vec<AdminRequestDurationBucket>,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AdminRequestDurationBucket {
    pub le: String,
    pub count: u64,
}

#[derive(Debug, Default)]
pub struct AdminRequestStats {
    series: RwLock<BTreeMap<AdminRequestKey, AdminRequestSeries>>,
}

#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd)]
struct AdminRequestKey {
    method: String,
    route: String,
    status_class: String,
}

#[derive(Debug, Clone, Default)]
struct AdminRequestSeries {
    total_requests: u64,
    duration_seconds_sum: f64,
    duration_seconds_buckets: [u64; ADMIN_REQUEST_DURATION_SECONDS_BUCKET_COUNT],
}

impl AdminRequestStats {
    pub fn shared() -> SharedAdminRequestStats {
        Arc::new(Self::default())
    }

    pub fn observe(&self, method: &str, route: &str, status_class: &str, duration: Duration) {
        let mut series = self.series.write();
        let item = series
            .entry(AdminRequestKey {
                method: method.to_string(),
                route: route.to_string(),
                status_class: status_class.to_string(),
            })
            .or_default();
        item.total_requests = item.total_requests.saturating_add(1);
        item.duration_seconds_sum += duration.as_secs_f64();
        item.duration_seconds_buckets[duration_seconds_bucket_index(duration)] += 1;
    }

    pub fn snapshot(&self) -> AdminRequestStatsSnapshot {
        let series = self
            .series
            .read()
            .iter()
            .map(|(key, value)| AdminRequestMetricSeries {
                method: key.method.clone(),
                route: key.route.clone(),
                status_class: key.status_class.clone(),
                total_requests: value.total_requests,
                duration_seconds_sum: value.duration_seconds_sum,
                duration_seconds_count: value.total_requests,
                duration_seconds_buckets: cumulative_duration_buckets(
                    &value.duration_seconds_buckets,
                ),
            })
            .collect();
        AdminRequestStatsSnapshot { series }
    }
}

fn duration_seconds_bucket_index(duration: Duration) -> usize {
    let seconds = duration.as_secs_f64();
    ADMIN_REQUEST_DURATION_SECONDS_BUCKETS
        .iter()
        .position(|(upper, _)| seconds <= *upper)
        .unwrap_or(ADMIN_REQUEST_DURATION_SECONDS_BUCKETS.len())
}

fn cumulative_duration_buckets(
    buckets: &[u64; ADMIN_REQUEST_DURATION_SECONDS_BUCKET_COUNT],
) -> Vec<AdminRequestDurationBucket> {
    let mut cumulative = 0;
    ADMIN_REQUEST_DURATION_SECONDS_BUCKETS
        .iter()
        .map(|(_, label)| *label)
        .chain(std::iter::once("+Inf"))
        .zip(buckets.iter().copied())
        .map(|(le, value)| {
            cumulative += value;
            AdminRequestDurationBucket {
                le: le.to_string(),
                count: cumulative,
            }
        })
        .collect()
}
