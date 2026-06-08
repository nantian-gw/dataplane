use std::collections::BTreeMap;

use super::HttpRateLimitRejection;

#[derive(Debug, Default)]
pub(super) struct HttpRateLimitStats {
    pub(super) allowed_total: u64,
    pub(super) rejected_total: u64,
    pub(super) rejected_global_total: u64,
    pub(super) rejected_listener_total: u64,
    pub(super) rejected_route_total: u64,
    pub(super) rejected_listener_by_name: BTreeMap<String, u64>,
    pub(super) rejected_route_by_name: BTreeMap<String, u64>,
}

impl HttpRateLimitStats {
    pub(super) fn observe_allow(&mut self) {
        self.allowed_total = self.allowed_total.saturating_add(1);
    }

    pub(super) fn observe_reject(&mut self, rejection: HttpRateLimitRejection, key: Option<&str>) {
        self.rejected_total = self.rejected_total.saturating_add(1);
        match rejection {
            HttpRateLimitRejection::Global => {
                self.rejected_global_total = self.rejected_global_total.saturating_add(1);
            }
            HttpRateLimitRejection::Listener => {
                self.rejected_listener_total = self.rejected_listener_total.saturating_add(1);
                if let Some(key) = key.filter(|value| !value.trim().is_empty()) {
                    increment_named(&mut self.rejected_listener_by_name, key);
                }
            }
            HttpRateLimitRejection::Route => {
                self.rejected_route_total = self.rejected_route_total.saturating_add(1);
                if let Some(key) = key.filter(|value| !value.trim().is_empty()) {
                    increment_named(&mut self.rejected_route_by_name, key);
                }
            }
        }
    }
}

fn increment_named(items: &mut BTreeMap<String, u64>, key: &str) {
    *items.entry(key.to_string()).or_default() += 1;
}
