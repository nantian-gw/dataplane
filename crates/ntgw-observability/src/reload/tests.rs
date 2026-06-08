use anyhow::{Result, anyhow};
use std::{
    fs,
    path::PathBuf,
    sync::Arc,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use super::ReloadingFile;

fn parse_trimmed_text(bytes: &[u8]) -> Result<Arc<str>> {
    let raw = std::str::from_utf8(bytes)?;
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err(anyhow!("text file is empty"));
    }
    Ok(Arc::<str>::from(trimmed))
}

#[test]
fn reloading_file_reloads_after_stamp_change() {
    let path = temp_path("reload");
    fs::write(&path, "old\n").expect("write initial file");

    let file = ReloadingFile::new(path.clone(), Duration::ZERO, parse_trimmed_text)
        .expect("reloadable file");
    assert_eq!(file.load().expect("load"), Arc::<str>::from("old"));

    fs::write(&path, "new\n").expect("rewrite file");
    assert_eq!(file.load().expect("reload"), Arc::<str>::from("new"));

    let _ = fs::remove_file(path);
}

#[test]
fn reloading_file_uses_cached_value_within_refresh_interval() {
    let path = temp_path("cache");
    fs::write(&path, "old\n").expect("write initial file");

    let file = ReloadingFile::new(path.clone(), Duration::from_secs(60), parse_trimmed_text)
        .expect("reloadable file");
    assert_eq!(file.load().expect("load"), Arc::<str>::from("old"));

    fs::write(&path, "new\n").expect("rewrite file");
    assert_eq!(file.load().expect("cached"), Arc::<str>::from("old"));

    let _ = fs::remove_file(path);
}

#[test]
fn lazy_reloading_file_initializes_on_first_load() {
    let path = temp_path("lazy");
    let file = ReloadingFile::new_lazy(path.clone(), Duration::ZERO, parse_trimmed_text);

    fs::write(&path, "lazy\n").expect("write file");
    assert_eq!(file.load().expect("load"), Arc::<str>::from("lazy"));

    let _ = fs::remove_file(path);
}

fn temp_path(prefix: &str) -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time")
        .as_nanos();
    std::env::temp_dir().join(format!("ntgw-observability-{prefix}-{unique}.txt"))
}
