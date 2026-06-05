use anyhow::Result;
use parking_lot::RwLock;
use std::{
    fs,
    path::PathBuf,
    sync::Arc,
    time::{Duration, Instant, SystemTime},
};

#[derive(Clone)]
pub struct ReloadingFile<T> {
    path: PathBuf,
    refresh_interval: Duration,
    parse: fn(&[u8]) -> Result<T>,
    cache: Arc<RwLock<Option<CachedFile<T>>>>,
}

#[derive(Clone)]
struct CachedFile<T> {
    value: T,
    stamp: Option<FileStamp>,
    checked_at: Instant,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct FileStamp {
    modified: SystemTime,
    len: u64,
}

impl<T: Clone> ReloadingFile<T> {
    pub fn new(
        path: PathBuf,
        refresh_interval: Duration,
        parse: fn(&[u8]) -> Result<T>,
    ) -> Result<Self> {
        let (value, stamp) = read_value(&path, parse)?;
        Ok(Self {
            path,
            refresh_interval,
            parse,
            cache: Arc::new(RwLock::new(Some(CachedFile {
                value,
                stamp,
                checked_at: Instant::now(),
            }))),
        })
    }

    pub fn new_lazy(
        path: PathBuf,
        refresh_interval: Duration,
        parse: fn(&[u8]) -> Result<T>,
    ) -> Self {
        Self {
            path,
            refresh_interval,
            parse,
            cache: Arc::new(RwLock::new(None)),
        }
    }

    pub fn load(&self) -> Result<T> {
        if let Some(cached) = self.cache.read().as_ref().cloned() {
            if cached.checked_at.elapsed() < self.refresh_interval {
                return Ok(cached.value);
            }
        }

        let mut cache = self.cache.write();
        if let Some(cached) = cache.as_ref().cloned() {
            if cached.checked_at.elapsed() < self.refresh_interval {
                return Ok(cached.value);
            }

            let current_stamp = file_stamp(&self.path);
            if cached.stamp.is_some() && cached.stamp == current_stamp {
                let value = cached.value.clone();
                *cache = Some(CachedFile {
                    checked_at: Instant::now(),
                    ..cached
                });
                return Ok(value);
            }
        }

        let (value, stamp) = read_value(&self.path, self.parse)?;
        *cache = Some(CachedFile {
            value: value.clone(),
            stamp,
            checked_at: Instant::now(),
        });
        Ok(value)
    }
}

fn read_value<T>(path: &PathBuf, parse: fn(&[u8]) -> Result<T>) -> Result<(T, Option<FileStamp>)> {
    let bytes = fs::read(path)?;
    let value = parse(bytes.as_slice())?;
    Ok((value, file_stamp(path)))
}

fn file_stamp(path: &PathBuf) -> Option<FileStamp> {
    let metadata = fs::metadata(path).ok()?;
    Some(FileStamp {
        modified: metadata.modified().ok()?,
        len: metadata.len(),
    })
}

#[cfg(test)]
mod tests;
