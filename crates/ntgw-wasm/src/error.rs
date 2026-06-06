use thiserror::Error;

#[derive(Error, Debug)]
pub enum WasmError {
    #[error("plugin not found: {0}")]
    PluginNotFound(String),
    #[error("failed to load plugin '{name}': {reason}")]
    LoadFailed { name: String, reason: String },
    #[error("plugin '{0}' execution failed: {1}")]
    PluginExecution(String, String),
    #[error("plugin '{0}' timed out")]
    PluginTimeout(String),
    #[error("plugin '{0}' rejected request with code {1}")]
    PluginRejected(String, i32),
    #[error("invalid hook: {0}")]
    InvalidHook(String),
    #[error("memory operation failed: {0}")]
    Memory(#[from] anyhow::Error),
    #[error("sandbox module not found: {0}")]
    SandboxModuleNotFound(String),
}
