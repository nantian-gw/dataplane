use std::collections::{BTreeMap, BTreeSet};

use anyhow::{anyhow, Context, Result};
use ntgw_ir::{Listener, SecretMaterial, Snapshot};
use pingora::tls::{nid::Nid, x509::X509};

use crate::RuntimeOptions;

const LISTENER_ADDRESSES_METADATA_KEY: &str = "nantian.dev/listener-addresses";

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ListenerPlan {
    pub(crate) binds: BTreeMap<String, PlannedSharedTlsBind>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PlannedSharedTlsBind {
    pub(crate) bind: String,
    pub(crate) terminate: Option<TerminateSurface>,
    pub(crate) passthrough: Option<PassthroughSurface>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TerminateSurface {
    pub(crate) listener_names: Vec<String>,
    pub(crate) identities: Vec<SharedTlsIdentity>,
    pub(crate) frontend_validation_mode: Option<String>,
    pub(crate) client_ca_bundle_pem: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PassthroughSurface {
    pub(crate) listener_names: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SharedTlsIdentity {
    pub(crate) secret_ref: String,
    pub(crate) cert_pem: String,
    pub(crate) key_pem: String,
    pub(crate) match_names: Vec<String>,
}

pub(crate) fn build_listener_plan(
    snapshot: &Snapshot,
    runtime: &RuntimeOptions,
) -> Result<ListenerPlan> {
    let mut binds = BTreeMap::new();

    for listener in &snapshot.listeners {
        let is_passthrough = is_tls_passthrough_listener(listener);
        let is_mixed = is_mixed_tls_listener(listener);
        let is_terminate = is_https_terminate_listener(listener);

        if !is_passthrough && !is_mixed && !is_terminate {
            continue;
        }

        // Step 1: Add passthrough surface for pure passthrough or mixed mode
        if is_passthrough || is_mixed {
            for bind in listener_bind_addrs(listener, runtime) {
                let shared = binds
                    .entry(bind.clone())
                    .or_insert_with(|| PlannedSharedTlsBind {
                        bind,
                        terminate: None,
                        passthrough: None,
                    });
                let passthrough = shared
                    .passthrough
                    .get_or_insert_with(|| PassthroughSurface {
                        listener_names: Vec::new(),
                    });
                passthrough.listener_names.push(listener.name.clone());
            }
        }

        // Step 2: Add terminate surface for HTTPS or mixed mode
        if !is_terminate && !is_mixed {
            continue;
        }

        let has_passthrough = is_mixed || is_tls_passthrough_listener(listener);

        let identities = resolve_tls_identities(listener, snapshot)?;
        if identities.is_empty() {
            continue;
        }

        let requested_frontend_validation_mode = frontend_validation_mode(listener);
        let requested_client_ca_bundle_pem = frontend_validation_bundle(listener);
        for bind in listener_bind_addrs(listener, runtime) {
            let shared = binds
                .entry(bind.clone())
                .or_insert_with(|| PlannedSharedTlsBind {
                    bind,
                    terminate: None,
                    passthrough: None,
                });

            // Skip if this bind already has a passthrough surface from a different
            // pure-passthrough listener (terminate cannot coexist with pure passthrough).
            if has_passthrough && shared.passthrough.is_some() && !is_mixed {
                continue;
            }

            let terminate = shared.terminate.get_or_insert_with(|| TerminateSurface {
                listener_names: Vec::new(),
                identities: Vec::new(),
                frontend_validation_mode: None,
                client_ca_bundle_pem: None,
            });
            merge_frontend_validation(
                terminate,
                requested_frontend_validation_mode.clone(),
                requested_client_ca_bundle_pem.clone(),
                shared.bind.as_str(),
            )?;
            terminate.listener_names.push(listener.name.clone());
            terminate.identities.extend(identities.clone());
        }
    }

    Ok(ListenerPlan { binds })
}

fn merge_frontend_validation(
    terminate: &mut TerminateSurface,
    requested_mode: Option<String>,
    requested_client_ca_bundle_pem: Option<String>,
    bind: &str,
) -> Result<()> {
    let requested_has_validation =
        requested_mode.is_some() || requested_client_ca_bundle_pem.is_some();
    if !requested_has_validation {
        return Ok(());
    }

    let current_has_validation =
        terminate.frontend_validation_mode.is_some() || terminate.client_ca_bundle_pem.is_some();
    if !current_has_validation {
        terminate.frontend_validation_mode = requested_mode;
        terminate.client_ca_bundle_pem = requested_client_ca_bundle_pem;
        return Ok(());
    }

    if terminate.frontend_validation_mode == requested_mode
        && terminate.client_ca_bundle_pem == requested_client_ca_bundle_pem
    {
        return Ok(());
    }

    Err(anyhow!(
        "frontend validation conflict on shared tls bind {bind}"
    ))
}

fn listener_bind_addrs(listener: &Listener, runtime: &RuntimeOptions) -> Vec<String> {
    let mut binds = BTreeSet::new();
    for address in listener_configured_addresses(listener) {
        for bind in bind_addrs(address.as_str(), listener.port, runtime.enable_ipv6) {
            binds.insert(bind);
        }
    }
    binds.into_iter().collect()
}

fn listener_configured_addresses(listener: &Listener) -> Vec<String> {
    if !listener.addresses.is_empty() {
        let mut out = Vec::new();
        let mut seen = BTreeSet::new();
        for value in listener
            .addresses
            .iter()
            .map(String::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            if seen.insert(value.to_string()) {
                out.push(value.to_string());
            }
        }
        if !out.is_empty() {
            return out;
        }
    }

    if let Some(raw) = listener.metadata.get(LISTENER_ADDRESSES_METADATA_KEY) {
        let mut out = Vec::new();
        let mut seen = BTreeSet::new();
        for value in raw
            .split(',')
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            if seen.insert(value.to_string()) {
                out.push(value.to_string());
            }
        }
        if !out.is_empty() {
            return out;
        }
    }

    if !listener.address.is_empty() {
        return vec![listener.address.clone()];
    }

    vec!["0.0.0.0".to_string()]
}

fn bind_addrs(address: &str, port: u32, enable_ipv6: bool) -> Vec<String> {
    let host = if address.is_empty() {
        "0.0.0.0"
    } else {
        address
    };

    if enable_ipv6 && host == "0.0.0.0" {
        return vec![format!("0.0.0.0:{port}"), format!("[::]:{port}")];
    }

    vec![socket_addr(host, port)]
}

fn socket_addr(address: &str, port: u32) -> String {
    if address.contains(':') && !address.starts_with('[') {
        format!("[{address}]:{port}")
    } else {
        format!("{address}:{port}")
    }
}

fn is_tls_passthrough_listener(listener: &Listener) -> bool {
    matches!(
        listener.protocol.as_str(),
        "LISTENER_PROTOCOL_TLS_PASSTHROUGH" | "TLS" | "TLS_PASSTHROUGH"
    ) && listener
        .tls
        .as_ref()
        .is_some_and(|tls| tls.enabled && tls.passthrough)
}

fn is_mixed_tls_listener(listener: &Listener) -> bool {
    listener.protocol.as_str() == "LISTENER_PROTOCOL_TLS"
        && listener
            .tls
            .as_ref()
            .is_some_and(|tls| tls.enabled && !tls.passthrough)
}

fn is_https_terminate_listener(listener: &Listener) -> bool {
    matches!(
        listener.protocol.as_str(),
        "LISTENER_PROTOCOL_HTTPS" | "HTTPS"
    ) && listener
        .tls
        .as_ref()
        .is_some_and(|tls| tls.enabled && !tls.passthrough)
}

fn frontend_validation_mode(listener: &Listener) -> Option<String> {
    let mode = listener
        .tls
        .as_ref()
        .and_then(|tls| tls.frontend_validation.as_ref())
        .map(|validation| validation.mode.trim().to_string())
        .unwrap_or_default();
    (!mode.is_empty()).then_some(mode)
}

fn frontend_validation_bundle(listener: &Listener) -> Option<String> {
    let bundle = listener
        .tls
        .as_ref()
        .and_then(|tls| tls.frontend_validation.as_ref())
        .map(|validation| {
            validation
                .ca_pems
                .iter()
                .filter(|item| !item.is_empty())
                .cloned()
                .collect::<Vec<_>>()
                .join("\n")
        })
        .unwrap_or_default();
    (!bundle.is_empty()).then_some(bundle)
}

fn resolve_tls_identities(
    listener: &Listener,
    snapshot: &Snapshot,
) -> Result<Vec<SharedTlsIdentity>> {
    let mut identities = Vec::new();
    let Some(tls) = listener.tls.as_ref() else {
        return Ok(identities);
    };

    for secret_ref in &tls.secret_refs {
        let Some(secret) = snapshot
            .secrets
            .iter()
            .find(|secret| format!("{}/{}", secret.namespace, secret.name) == *secret_ref)
        else {
            continue;
        };
        identities.push(build_tls_identity(secret_ref, secret)?);
    }

    Ok(identities)
}

fn build_tls_identity(secret_ref: &str, secret: &SecretMaterial) -> Result<SharedTlsIdentity> {
    let certs =
        X509::stack_from_pem(secret.cert_pem.as_bytes()).context("parse certificate PEM")?;
    let Some(leaf) = certs.first() else {
        return Err(anyhow!("no certificates found in PEM"));
    };
    pingora::tls::pkey::PKey::private_key_from_pem(secret.key_pem.as_bytes())
        .context("parse private key PEM")?;

    Ok(SharedTlsIdentity {
        secret_ref: secret_ref.to_string(),
        cert_pem: secret.cert_pem.clone(),
        key_pem: secret.key_pem.clone(),
        match_names: certificate_match_names(leaf),
    })
}

fn certificate_match_names(cert: &X509) -> Vec<String> {
    let mut names = Vec::new();

    if let Some(subject_alt_names) = cert.subject_alt_names() {
        for san in subject_alt_names {
            if let Some(dns_name) = san.dnsname() {
                let normalized = normalize_tls_server_name(dns_name);
                if !normalized.is_empty() && !names.contains(&normalized) {
                    names.push(normalized);
                }
            }
        }
    }

    if names.is_empty() {
        if let Some(common_name) = cert
            .subject_name()
            .entries_by_nid(Nid::COMMONNAME)
            .next()
            .and_then(|entry| entry.data().as_utf8().ok())
        {
            let normalized = normalize_tls_server_name(common_name.as_ref());
            if !normalized.is_empty() {
                names.push(normalized);
            }
        }
    }

    names
}

fn normalize_tls_server_name(value: &str) -> String {
    value.trim().trim_end_matches('.').to_ascii_lowercase()
}
