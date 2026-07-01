use super::*;
use anyhow::{Context, Result, anyhow};
use pingora::tls::{nid::Nid, x509::X509};

mod assets;
mod binds;

#[cfg(test)]
pub(super) use self::assets::materialize_tls_assets;
pub(super) use self::assets::{
    TlsAssetWriteStats, cleanup_unused_tls_assets_in_dir, materialize_runtime_plan,
    materialize_tls_assets_in_dir, referenced_tls_asset_prefixes, unique_asset_dir_name,
};
#[cfg(test)]
pub(super) use self::binds::bind_addrs;
pub(super) use self::binds::{
    bind_variants, is_http3_protocol, is_l7_protocol, listener_bind_addrs,
    tcp_socket_options_for_bind,
};
use self::binds::{is_https_protocol, is_plain_http_protocol};

#[derive(Debug, Clone)]
pub(super) struct RuntimePlan {
    pub(super) listeners: Vec<RuntimeListener>,
}

#[derive(Debug, Clone)]
pub(super) struct RuntimeListener {
    pub(super) name: String,
    pub(super) bind: String,
    pub(super) protocol: RuntimeListenerProtocol,
}

#[derive(Debug, Clone)]
pub(super) enum RuntimeListenerProtocol {
    Plain,
    Tls {
        #[cfg(test)]
        cert_path: String,
        #[cfg(test)]
        key_path: String,
        identities: Vec<TlsIdentity>,
        client_ca_path: Option<String>,
        frontend_validation_mode: Option<String>,
    },
}

pub(super) fn desired_listener_protocol(
    listener: &Listener,
    snapshot: &Snapshot,
    runtime: &RuntimeOptions,
) -> Option<ListenerProtocol> {
    if is_plain_http_protocol(&listener.protocol) {
        return Some(ListenerProtocol::Plain);
    }

    if !is_https_protocol(&listener.protocol) {
        return None;
    }

    let tls = listener.tls.as_ref()?;
    if !tls.enabled || tls.passthrough {
        return None;
    }

    let identities = resolve_tls_identities(listener, snapshot, tls);
    if identities.is_empty() {
        return None;
    }
    let min_version = if tls.min_version.is_empty() {
        runtime.tls_min_version.clone()
    } else {
        tls.min_version.clone()
    };
    let max_version = if tls.max_version.is_empty() {
        runtime.tls_max_version.clone()
    } else {
        tls.max_version.clone()
    };

    if min_version != "1.2" || max_version != "1.3" {
        warn!(
            listener = %listener.name,
            min_version = %min_version,
            max_version = %max_version,
            "current listener runtime does not enforce custom TLS version bounds yet"
        );
    }

    Some(ListenerProtocol::Tls(TlsMaterial {
        identities,
        min_version,
        max_version,
        client_ca_bundle_pem: tls
            .frontend_validation
            .as_ref()
            .and_then(frontend_validation_bundle),
        frontend_validation_mode: tls
            .frontend_validation
            .as_ref()
            .and_then(frontend_validation_mode),
    }))
}

fn frontend_validation_bundle(validation: &ntgw_ir::FrontendValidation) -> Option<String> {
    let bundle = validation
        .ca_pems
        .iter()
        .filter(|item| !item.is_empty())
        .cloned()
        .collect::<Vec<_>>()
        .join("\n");

    (!bundle.is_empty()).then_some(bundle)
}

fn frontend_validation_mode(validation: &ntgw_ir::FrontendValidation) -> Option<String> {
    let mode = validation.mode.trim();
    (!mode.is_empty()).then(|| mode.to_string())
}

fn validate_tls_identity(cert_pem: &str, key_pem: &str) -> Result<()> {
    let certs = X509::stack_from_pem(cert_pem.as_bytes()).context("parse certificate PEM")?;
    if certs.is_empty() {
        return Err(anyhow!("no certificates found in PEM"));
    }
    pingora::tls::pkey::PKey::private_key_from_pem(key_pem.as_bytes())
        .map(|_| ())
        .context("parse private key PEM")
}

fn resolve_tls_identities(
    listener: &Listener,
    snapshot: &Snapshot,
    tls: &TlsConfig,
) -> Vec<TlsIdentity> {
    let mut identities = Vec::new();

    for secret_ref in &tls.secret_refs {
        let Some(secret) = snapshot
            .secrets
            .iter()
            .find(|secret| format!("{}/{}", secret.namespace, secret.name) == *secret_ref)
            .filter(|secret| !secret.cert_pem.is_empty() && !secret.key_pem.is_empty())
        else {
            continue;
        };

        match build_tls_identity(secret_ref, secret) {
            Ok(identity) => identities.push(identity),
            Err(err) => {
                warn!(
                    listener = %listener.name,
                    secret = %secret_ref,
                    error = %err,
                    "skipping invalid certificate material referenced by tls-terminating http listener"
                );
            }
        }
    }

    identities
}

fn build_tls_identity(secret_ref: &str, secret: &SecretMaterial) -> Result<TlsIdentity> {
    validate_tls_identity(secret.cert_pem.as_str(), secret.key_pem.as_str())?;

    let certs =
        X509::stack_from_pem(secret.cert_pem.as_bytes()).context("parse certificate PEM")?;
    let Some(leaf) = certs.first() else {
        return Err(anyhow!("no certificates found in PEM"));
    };

    Ok(TlsIdentity {
        secret_ref: secret_ref.to_string(),
        cert_pem: secret.cert_pem.clone(),
        key_pem: secret.key_pem.clone(),
        match_names: certificate_match_names(leaf),
    })
}

fn certificate_match_names(cert: &X509) -> Vec<String> {
    let mut names = BTreeSet::new();

    if let Some(subject_alt_names) = cert.subject_alt_names() {
        for san in subject_alt_names {
            if let Some(dns_name) = san.dnsname() {
                let normalized = normalize_tls_server_name(dns_name);
                if !normalized.is_empty() {
                    names.insert(normalized);
                }
            }
        }
    }

    if names.is_empty()
        && let Some(common_name) = cert
            .subject_name()
            .entries_by_nid(Nid::COMMONNAME)
            .next()
            .and_then(|entry| entry.data().to_string().ok())
    {
        let normalized = normalize_tls_server_name(common_name.as_ref());
        if !normalized.is_empty() {
            names.insert(normalized);
        }
    }

    names.into_iter().collect()
}

fn normalize_tls_server_name(value: &str) -> String {
    value.trim().trim_end_matches('.').to_ascii_lowercase()
}

fn tls_identity_match_rank(identity: &TlsIdentity, server_name: &str) -> Option<u8> {
    let normalized = normalize_tls_server_name(server_name);
    let mut best = None;

    for pattern in &identity.match_names {
        if pattern == &normalized {
            return Some(2);
        }
        if wildcard_hostname_matches(pattern, &normalized) {
            best = Some(best.unwrap_or(1));
        }
    }

    best
}

pub(super) fn wildcard_hostname_matches(pattern: &str, host: &str) -> bool {
    let Some(suffix) = pattern.strip_prefix("*.") else {
        return false;
    };

    if host == suffix || !host.ends_with(suffix) {
        return false;
    }

    let Some(prefix) = host
        .strip_suffix(suffix)
        .and_then(|value| value.strip_suffix('.'))
    else {
        return false;
    };

    !prefix.is_empty() && !prefix.contains('.')
}

pub(super) fn ordered_tls_identity_candidates<'a>(
    identities: &'a [TlsIdentity],
    server_name: Option<&str>,
) -> Vec<&'a TlsIdentity> {
    if identities.is_empty() {
        return Vec::new();
    }

    let mut scored = Vec::new();
    let mut fallback = Vec::new();

    for (index, identity) in identities.iter().enumerate() {
        if let Some(server_name) = server_name
            && let Some(rank) = tls_identity_match_rank(identity, server_name)
        {
            scored.push((rank, index, identity));
            continue;
        }
        fallback.push((index, identity));
    }

    scored.sort_by(|left, right| right.0.cmp(&left.0).then(left.1.cmp(&right.1)));

    let mut ordered = Vec::with_capacity(identities.len());
    ordered.extend(scored.into_iter().map(|(_, _, identity)| identity));
    ordered.extend(fallback.into_iter().map(|(_, identity)| identity));
    ordered
}
