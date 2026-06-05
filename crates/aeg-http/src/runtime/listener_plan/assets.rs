use std::{
    collections::BTreeSet,
    fs,
    io::Write,
    path::{Path, PathBuf},
};

use anyhow::{anyhow, Result};
use sha2::{Digest, Sha256};

#[cfg(unix)]
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};

use super::{
    ListenerPlan, ListenerProtocol, RuntimeListener, RuntimeListenerProtocol, RuntimePlan,
    TlsIdentity, TlsMaterial,
};

const TLS_ASSET_TEMP_FILE_MARKER: &str = ".aeg-tls-asset-tmp-";

pub(in crate::runtime) fn materialize_runtime_plan(
    plan: &ListenerPlan,
    asset_dir: &Path,
) -> RuntimePlan {
    RuntimePlan {
        listeners: plan
            .listeners
            .iter()
            .filter_map(|listener| {
                let protocol = match &listener.protocol {
                    ListenerProtocol::Plain => RuntimeListenerProtocol::Plain,
                    ListenerProtocol::Tls(material) => {
                        let primary = primary_tls_identity(material)?;
                        let prefix = tls_asset_prefix(material, primary);
                        RuntimeListenerProtocol::Tls {
                            #[cfg(test)]
                            cert_path: asset_dir
                                .join(format!("{prefix}.crt"))
                                .display()
                                .to_string(),
                            #[cfg(test)]
                            key_path: asset_dir
                                .join(format!("{prefix}.key"))
                                .display()
                                .to_string(),
                            client_ca_path: material.client_ca_bundle_pem.as_ref().map(|_| {
                                asset_dir
                                    .join(format!("{prefix}.ca.crt"))
                                    .display()
                                    .to_string()
                            }),
                            frontend_validation_mode: material.frontend_validation_mode.clone(),
                            identities: material.identities.clone(),
                        }
                    }
                };

                Some(RuntimeListener {
                    name: listener.name.clone(),
                    bind: listener.bind.clone(),
                    protocol,
                })
            })
            .collect(),
    }
}

#[cfg(test)]
pub(in crate::runtime) fn materialize_tls_assets(plan: &ListenerPlan) -> Result<PathBuf> {
    let root = std::env::temp_dir()
        .join("nantian-gw")
        .join("http-listeners")
        .join(unique_asset_dir_name());
    materialize_tls_assets_in_dir(plan, &root)?;
    Ok(root)
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(in crate::runtime) struct TlsAssetWriteStats {
    pub(in crate::runtime) reused: u64,
}

pub(in crate::runtime) fn materialize_tls_assets_in_dir(
    plan: &ListenerPlan,
    root: &Path,
) -> Result<TlsAssetWriteStats> {
    ensure_tls_asset_dir(root)?;
    cleanup_stale_tls_temp_files_in_dir(root)?;
    let mut written = BTreeSet::new();
    let mut reused = 0u64;

    for listener in &plan.listeners {
        let ListenerProtocol::Tls(material) = &listener.protocol else {
            continue;
        };

        let Some(primary) = primary_tls_identity(material) else {
            continue;
        };
        let prefix = tls_asset_prefix(material, primary);
        if !written.insert(prefix.clone()) {
            reused += 1;
            continue;
        }

        let cert_path = root.join(format!("{prefix}.crt"));
        let key_path = root.join(format!("{prefix}.key"));
        let client_ca_path = material
            .client_ca_bundle_pem
            .as_ref()
            .map(|_| root.join(format!("{prefix}.ca.crt")));
        if tls_asset_files_exist(&cert_path, &key_path, client_ca_path.as_deref()) {
            reused += 1;
            continue;
        }

        atomic_write_tls_asset(&cert_path, primary.cert_pem.as_bytes())?;
        atomic_write_tls_asset(&key_path, primary.key_pem.as_bytes())?;
        if let Some(client_ca_bundle_pem) = &material.client_ca_bundle_pem {
            let client_ca_path = client_ca_path
                .as_deref()
                .ok_or_else(|| anyhow!("client CA bundle path missing for {prefix}"))?;
            atomic_write_tls_asset(client_ca_path, client_ca_bundle_pem.as_bytes())?;
        }
    }

    Ok(TlsAssetWriteStats { reused })
}

pub(in crate::runtime) fn referenced_tls_asset_prefixes(plan: &ListenerPlan) -> BTreeSet<String> {
    plan.listeners
        .iter()
        .filter_map(|listener| match &listener.protocol {
            ListenerProtocol::Plain => None,
            ListenerProtocol::Tls(material) => {
                let primary = primary_tls_identity(material)?;
                Some(tls_asset_prefix(material, primary))
            }
        })
        .collect()
}

pub(in crate::runtime) fn cleanup_unused_tls_assets_in_dir(
    root: &Path,
    referenced_prefixes: &BTreeSet<String>,
) -> Result<()> {
    if !root.is_dir() {
        return Ok(());
    }

    for entry in fs::read_dir(root)? {
        let entry = entry?;
        let path = entry.path();
        if !path.is_file() {
            continue;
        }

        let Some(name) = path.file_name().and_then(|item| item.to_str()) else {
            continue;
        };
        if is_tls_asset_temp_file_name(name) {
            fs::remove_file(&path)?;
            continue;
        }
        let Some(prefix) = tls_asset_prefix_from_file_name(name) else {
            continue;
        };
        if referenced_prefixes.contains(prefix) {
            continue;
        }

        fs::remove_file(&path)?;
    }

    Ok(())
}

fn cleanup_stale_tls_temp_files_in_dir(root: &Path) -> Result<()> {
    if !root.is_dir() {
        return Ok(());
    }

    for entry in fs::read_dir(root)? {
        let entry = entry?;
        let path = entry.path();
        if !path.is_file() {
            continue;
        }

        let Some(name) = path.file_name().and_then(|item| item.to_str()) else {
            continue;
        };
        if is_tls_asset_temp_file_name(name) {
            fs::remove_file(path)?;
        }
    }

    Ok(())
}

fn is_tls_asset_temp_file_name(name: &str) -> bool {
    name.starts_with(TLS_ASSET_TEMP_FILE_MARKER)
}

fn atomic_write_tls_asset(path: &Path, contents: &[u8]) -> Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| anyhow!("tls asset path {} has no parent directory", path.display()))?;
    ensure_tls_asset_dir(parent)?;

    let temp_path = new_tls_asset_temp_path(parent);
    let write_result = (|| -> Result<()> {
        let mut options = fs::OpenOptions::new();
        options.create_new(true).write(true);
        #[cfg(unix)]
        options.mode(0o600);
        let mut file = options.open(&temp_path)?;
        file.write_all(contents)?;
        file.sync_all()?;
        drop(file);
        fs::rename(&temp_path, path)?;
        Ok(())
    })();

    if write_result.is_err() {
        let _ = fs::remove_file(&temp_path);
    }

    write_result
}

fn ensure_tls_asset_dir(path: &Path) -> Result<()> {
    fs::create_dir_all(path)?;
    #[cfg(unix)]
    fs::set_permissions(path, fs::Permissions::from_mode(0o700))?;
    Ok(())
}

fn new_tls_asset_temp_path(parent: &Path) -> PathBuf {
    for _ in 0..16 {
        let candidate = parent.join(format!(
            "{TLS_ASSET_TEMP_FILE_MARKER}{}-{}",
            std::process::id(),
            unique_asset_dir_name()
        ));
        if !candidate.exists() {
            return candidate;
        }
    }

    parent.join(format!(
        "{TLS_ASSET_TEMP_FILE_MARKER}{}-fallback",
        unique_asset_dir_name()
    ))
}

fn tls_asset_files_exist(cert_path: &Path, key_path: &Path, client_ca_path: Option<&Path>) -> bool {
    cert_path.is_file() && key_path.is_file() && client_ca_path.is_none_or(|path| path.is_file())
}

fn tls_asset_prefix_from_file_name(name: &str) -> Option<&str> {
    name.strip_suffix(".ca.crt")
        .or_else(|| name.strip_suffix(".crt"))
        .or_else(|| name.strip_suffix(".key"))
}

fn tls_asset_prefix(material: &TlsMaterial, primary: &TlsIdentity) -> String {
    let mut hasher = Sha256::new();
    hasher.update(primary.secret_ref.as_bytes());
    hasher.update([0]);
    hasher.update(primary.cert_pem.as_bytes());
    hasher.update([0]);
    hasher.update(primary.key_pem.as_bytes());
    hasher.update([0]);
    if let Some(bundle) = &material.client_ca_bundle_pem {
        hasher.update(bundle.as_bytes());
    }

    format!(
        "{}-{}",
        sanitize_ref(&primary.secret_ref),
        short_digest(&hasher.finalize())
    )
}

fn primary_tls_identity(material: &TlsMaterial) -> Option<&TlsIdentity> {
    material.identities.first()
}

fn short_digest(bytes: &[u8]) -> String {
    bytes
        .iter()
        .take(6)
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

pub(in crate::runtime) fn unique_asset_dir_name() -> String {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    format!("runtime-{nanos}")
}

fn sanitize_ref(value: &str) -> String {
    value
        .chars()
        .map(|ch| if ch.is_ascii_alphanumeric() { ch } else { '_' })
        .collect()
}
