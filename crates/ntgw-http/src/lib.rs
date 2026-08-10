#![forbid(unsafe_code)]

pub mod bench;
pub mod cache;
mod extensions;
mod filters;
mod mirror;
pub mod proxy;
pub mod runtime;
pub mod session;

pub use cache::{CacheManager, CacheOptions};

pub use mirror::configure_request_mirror_budget;
pub use ntgw_observability::AccessLogOptions;
pub use proxy::{DownstreamTlsInfo, RequestContext, UpstreamTuningOptions};
pub use runtime::bench as runtime_bench;
pub use runtime::{
    AcceptedHttpApp, HttpCapacityOptions, ReloadableRuntimeConfig, RuntimeOptions, build_http_app,
    http3_available, process_accepted_stream, spawn,
};
pub use session::{SessionManager, SessionPersistenceOptions};