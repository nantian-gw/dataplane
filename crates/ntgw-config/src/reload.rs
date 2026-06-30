use std::{
    fs,
    path::{Path, PathBuf},
    sync::{Arc, RwLock},
    time::{Duration, Instant, SystemTime},
};

use anyhow::Result;

use crate::DataPlaneConfig;

#[derive(Clone)]
pub struct ReloadingDataPlaneConfig {
    path: PathBuf,
    refresh_interval: Duration,
    cache: Arc<RwLock<CachedConfig>>,
}

#[derive(Clone)]
struct CachedConfig {
    value: DataPlaneConfig,
    stamp: Option<FileStamp>,
    checked_at: Instant,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct FileStamp {
    modified: SystemTime,
    len: u64,
}

impl ReloadingDataPlaneConfig {
    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn new(path: impl AsRef<Path>, refresh_interval: Duration) -> Result<Self> {
        let path = path.as_ref().to_path_buf();
        let value = DataPlaneConfig::load(&path)?;
        let stamp = file_stamp(&path);
        Ok(Self {
            path,
            refresh_interval,
            cache: Arc::new(RwLock::new(CachedConfig {
                value,
                stamp,
                checked_at: Instant::now(),
            })),
        })
    }

    pub fn load(&self) -> Result<DataPlaneConfig> {
        self.refresh(true).map(|value| {
            value.unwrap_or_else(|| {
                self.cache
                    .read()
                    .unwrap_or_else(|err| err.into_inner())
                    .value
                    .clone()
            })
        })
    }

    pub fn load_if_changed(&self) -> Result<Option<DataPlaneConfig>> {
        self.refresh(false)
    }

    fn refresh(&self, return_cached: bool) -> Result<Option<DataPlaneConfig>> {
        {
            let cached = self.cache.read().unwrap_or_else(|err| err.into_inner());
            if cached.checked_at.elapsed() < self.refresh_interval {
                return Ok(return_cached.then(|| cached.value.clone()));
            }
        }

        let mut cached = self.cache.write().unwrap_or_else(|err| err.into_inner());
        if cached.checked_at.elapsed() < self.refresh_interval {
            return Ok(return_cached.then(|| cached.value.clone()));
        }

        let current_stamp = file_stamp(&self.path);
        if cached.stamp.is_some() && cached.stamp == current_stamp {
            cached.checked_at = Instant::now();
            return Ok(return_cached.then(|| cached.value.clone()));
        }

        let value = DataPlaneConfig::load(&self.path)?;
        cached.value = value.clone();
        cached.stamp = current_stamp;
        cached.checked_at = Instant::now();
        Ok(Some(value))
    }
}

fn file_stamp(path: &Path) -> Option<FileStamp> {
    let metadata = fs::metadata(path).ok()?;
    Some(FileStamp {
        modified: metadata.modified().ok()?,
        len: metadata.len(),
    })
}
