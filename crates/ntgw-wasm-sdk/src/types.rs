use crate::host_get_header;
use crate::host_log;
use crate::host_set_header;

/// Result type for plugin functions
pub type PluginResult = Result<(), i32>;

/// Log levels for the upstream::log host function
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogLevel {
    Info = 0,
    Warn = 1,
    Error = 2,
}

/// Plugin initialization config (passed as JSON string from CRD)
pub struct PluginConfig<'a> {
    pub raw: &'a str,
}

/// Request phase context passed to plugin hooks
pub struct RequestContext {
    /// Whether any modification has been made
    pub modified: bool,
}

impl RequestContext {
    pub fn new() -> Self {
        Self { modified: false }
    }

    /// Get a request header by name
    pub fn get_header(&self, name: &str) -> Option<String> {
        unsafe { host_get_header(name) }
    }

    /// Set a header (visible in subsequent plugins, not upstream)
    pub fn set_header(&mut self, name: &str, value: &str) {
        unsafe { host_set_header(name, value) }
        self.modified = true;
    }

    /// Log a message through the gateway's tracing system
    pub fn log(&self, level: LogLevel, msg: &str) {
        unsafe { host_log(level, msg) }
    }
}

impl Default for RequestContext {
    fn default() -> Self {
        Self::new()
    }
}
