#![forbid(unsafe_code)]

pub mod bench;
pub mod cache;
mod extensions;
mod filters;
mod mirror;
mod proxy;
mod runtime;
mod session;

pub use cache::{CacheManager, CacheOptions};

pub use aeg_observability::AccessLogOptions;
pub use mirror::configure_request_mirror_budget;
pub use proxy::{DownstreamTlsInfo, RequestContext};
pub use runtime::bench as runtime_bench;
pub use runtime::{
    build_http_app, http3_available, process_accepted_stream, spawn, AcceptedHttpApp,
    HttpCapacityOptions, ReloadableRuntimeConfig, RuntimeOptions,
};
pub use session::{SessionManager, SessionPersistenceOptions};
