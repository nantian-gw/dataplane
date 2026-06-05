use std::{borrow::Cow, fmt::Write as _};

const FNV_OFFSET_BASIS: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

#[derive(Debug, Clone, Copy)]
pub(super) struct BackendNameParts<'a> {
    pub(super) namespace: &'a str,
    pub(super) name: &'a str,
    pub(super) port: u32,
}

pub(super) fn parse_backend_name_ref(value: &str) -> Option<BackendNameParts<'_>> {
    let (namespace, rest) = value.split_once('/')?;
    let (name, port) = rest.rsplit_once(':')?;
    let port = port.parse().ok()?;
    Some(BackendNameParts {
        namespace,
        name,
        port,
    })
}

pub(super) fn canonical_route_kind_ref(value: &str) -> Cow<'static, str> {
    let trimmed = value.trim();
    if trimmed.eq_ignore_ascii_case("HTTP") || trimmed.eq_ignore_ascii_case("HTTPRoute") {
        return Cow::Borrowed("HTTPRoute");
    }
    if trimmed.eq_ignore_ascii_case("GRPC") || trimmed.eq_ignore_ascii_case("GRPCRoute") {
        return Cow::Borrowed("GRPCRoute");
    }
    if trimmed.eq_ignore_ascii_case("TCP") || trimmed.eq_ignore_ascii_case("TCPRoute") {
        return Cow::Borrowed("TCPRoute");
    }
    if trimmed.eq_ignore_ascii_case("UDP") || trimmed.eq_ignore_ascii_case("UDPRoute") {
        return Cow::Borrowed("UDPRoute");
    }
    if trimmed.eq_ignore_ascii_case("TLS") || trimmed.eq_ignore_ascii_case("TLSRoute") {
        return Cow::Borrowed("TLSRoute");
    }

    let normalized = trimmed.to_ascii_uppercase();
    let other = normalized.as_str();
    if other.ends_with("ROUTE") && !other.is_empty() {
        let base = &other[..other.len() - "ROUTE".len()];
        Cow::Owned(format!("{}Route", title_case(base)))
    } else {
        Cow::Owned(format!("{}Route", title_case(&normalized)))
    }
}

fn title_case(value: &str) -> String {
    let lower = value.to_ascii_lowercase();
    let mut chars = lower.chars();
    let Some(first) = chars.next() else {
        return String::new();
    };
    let mut out = String::new();
    out.push(first.to_ascii_uppercase());
    out.extend(chars);
    out
}

pub(super) fn edge_id(source: &str, target: &str) -> String {
    let mut out = String::with_capacity("edge:".len() + source.len() + 1 + target.len());
    out.push_str("edge:");
    out.push_str(source);
    out.push(':');
    out.push_str(target);
    out
}

pub(super) fn topology_shard_key(listener_id: &str, route_id: &str, backend_name: &str) -> u64 {
    let mut hash = FNV_OFFSET_BASIS;
    hash = write_hash_part(hash, listener_id);
    hash = write_hash_part(hash, route_id);
    hash = write_hash_part(hash, backend_name);
    hash
}

pub(super) fn listener_node_id(name: &str) -> String {
    let mut out = String::with_capacity("listener:".len() + name.len());
    out.push_str("listener:");
    out.push_str(name);
    out
}

pub(super) fn route_node_id(kind: &str, namespace: &str, name: &str) -> String {
    let mut out =
        String::with_capacity("route:".len() + kind.len() + 1 + namespace.len() + 1 + name.len());
    out.push_str("route:");
    out.push_str(kind);
    out.push(':');
    out.push_str(namespace);
    out.push('/');
    out.push_str(name);
    out
}

pub(super) fn backend_node_id(namespace: &str, name: &str, port: u32) -> String {
    let mut out = String::with_capacity(
        "backend:".len() + namespace.len() + 1 + name.len() + 1 + decimal_len(port),
    );
    out.push_str("backend:");
    out.push_str(namespace);
    out.push('/');
    out.push_str(name);
    out.push(':');
    let _ = write!(&mut out, "{port}");
    out
}

pub(super) fn endpoint_set_node_id(namespace: &str, name: &str, port: u32) -> String {
    let mut out = String::with_capacity(
        "endpoint-set:".len() + namespace.len() + 1 + name.len() + 1 + decimal_len(port),
    );
    out.push_str("endpoint-set:");
    out.push_str(namespace);
    out.push('/');
    out.push_str(name);
    out.push(':');
    let _ = write!(&mut out, "{port}");
    out
}

fn decimal_len(value: u32) -> usize {
    if value == 0 {
        return 1;
    }
    value.ilog10() as usize + 1
}

fn write_hash_part(mut hash: u64, value: &str) -> u64 {
    hash = write_hash_bytes(hash, &(value.len() as u64).to_le_bytes());
    write_hash_bytes(hash, value.as_bytes())
}

fn write_hash_bytes(mut hash: u64, bytes: &[u8]) -> u64 {
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    hash
}
