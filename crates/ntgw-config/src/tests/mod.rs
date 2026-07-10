use std::{fs, path::PathBuf, time::Duration};

use super::{
    AdminAuthConfig, DataPlaneConfig, SessionPersistenceConfig, XdsTlsConfig, trimmed_non_empty,
};

mod basics;
mod config_load;
mod logging;
mod route_policy;
mod runtime_protection;
mod runtime_tuning;
mod session_persistence;
mod xds_transport;

fn tempfile_dir() -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "ntgw-config-test-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("time should be monotonic")
            .as_nanos()
    ));
    fs::create_dir_all(&dir).expect("temp dir should be created");
    dir
}
