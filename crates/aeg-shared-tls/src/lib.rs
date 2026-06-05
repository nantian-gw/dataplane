#![forbid(unsafe_code)]

mod dispatch;
mod listener_plan;
mod preface;
mod runtime;

#[cfg(test)]
mod tests;

pub use runtime::{run, ReloadableRuntimeConfig};

#[derive(Debug, Clone)]
pub struct RuntimeOptions {
    pub enable_ipv6: bool,
    pub reload_retry_interval: std::time::Duration,
}

impl Default for RuntimeOptions {
    fn default() -> Self {
        Self {
            enable_ipv6: false,
            reload_retry_interval: std::time::Duration::from_secs(1),
        }
    }
}
