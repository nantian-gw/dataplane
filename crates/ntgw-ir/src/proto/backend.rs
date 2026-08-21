use super::routes::session_persistence_from_proto;
use super::values::{duration_from_proto, list_value, optional_string, struct_value};
use super::*;
use crate::{CircuitBreakerConfig, HealthCheckConfig, OutlierDetectionConfig, SlowStartConfig};

pub(super) fn backend_from_proto(item: proto::BackendCluster) -> BackendCluster {
    let wasm_plugin = item.wasm_plugin.map(|wp| WasmPluginConfig {
        name: wp.name,
        namespace: wp.namespace,
        wasm_bytes: wp.wasm_bytes,
        sha256: wp.sha256,
        hooks: wp.hooks,
        config_json: wp.config_json,
        source_url: wp.source_url,
        sandbox: WasmSandboxConfig {
            max_memory_bytes: wp.sandbox.map(|s| s.max_memory_bytes).unwrap_or(0),
            max_execution_time_ms: wp.sandbox.map(|s| s.max_execution_time_ms).unwrap_or(0),
            allow_network: wp.sandbox.map(|s| s.allow_network).unwrap_or(false),
            allow_file_system: wp.sandbox.map(|s| s.allow_file_system).unwrap_or(false),
        },
    });

    let ai_service = item.ai_service.map(|ai| AIServiceConfig {
        provider: ai.provider,
        format: ai.format,
        model: ai.model,
        endpoint: ai.endpoint,
        auth: ai.auth.map(|a| AIServiceAuthConfig {
            auth_type: a.r#type,
            secret_ref: a.secret_ref,
            key: a.key,
            header: a.header,
        }),
        timeout_secs: ai
            .timeout
            .as_ref()
            .and_then(duration_from_proto)
            .map(|d| d.as_secs()),
        retry_max_retries: ai.retry_max_retries,
        retry_backoff_ms: ai
            .retry_backoff
            .as_ref()
            .and_then(duration_from_proto)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0),
    });

    let token_policy = item.token_policy.map(|tp| TokenPolicyConfig {
        tokens_per_minute: tp.tokens_per_minute,
        tokens_per_hour: tp.tokens_per_hour,
        requests_per_minute: tp.requests_per_minute,
        scope: tp.scope,
        burst: tp.burst,
        on_limit: tp.on_limit,
    });

    let circuit_breaker = item.circuit_breaker.map(|cb| CircuitBreakerConfig {
        max_inflight_requests: cb.max_inflight_requests,
    });

    BackendCluster {
        name: item.name,
        namespace: item.namespace,
        protocol: item.protocol,
        endpoints: item
            .endpoints
            .into_iter()
            .map(|endpoint| BackendEndpoint {
                address: endpoint.address,
                port: endpoint.port,
                healthy: endpoint.healthy,
            })
            .collect(),
        wasm_plugin,
        ai_service,
        token_policy,
        circuit_breaker,
        security_policy: item.security_policy.map(security_policy_from_proto),
    }
}

pub(super) fn workloads_from_extensions(extensions: Option<&Struct>) -> Vec<Workload> {
    list_value(extensions.and_then(|item| item.fields.get("workloads")))
        .map(|list| {
            list.values
                .iter()
                .filter_map(workload_from_value)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default()
}

fn workload_from_value(value: &Value) -> Option<Workload> {
    let fields = struct_value(value)?;
    Some(Workload {
        namespace: optional_string(fields.fields.get("namespace"))?,
        name: optional_string(fields.fields.get("name"))?,
        ip: optional_string(fields.fields.get("ip"))?,
    })
}

pub(super) fn backend_policy_from_proto(item: &proto::BackendCluster) -> BackendPolicy {
    BackendPolicy {
        connect_timeout: item.connect_timeout.as_ref().and_then(duration_from_proto),
        request_timeout: item
            .request_timeout
            .as_ref()
            .and_then(duration_from_proto)
            .filter(|timeout| !timeout.is_zero()),
        tls_validation: item
            .tls_validation
            .clone()
            .map(backend_tls_validation_from_proto),
        session_persistence: item
            .session_persistence
            .clone()
            .map(session_persistence_from_proto),
        load_balancing: item.load_balancing.clone().map(load_balancing_from_proto),
        health_check: item.health_check.as_ref().map(|hc| HealthCheckConfig {
            r#type: hc.r#type.clone(),
            path: hc.path.clone(),
            expected_status: hc.expected_status,
            interval: hc.interval.as_ref().and_then(duration_from_proto),
            timeout: hc.timeout.as_ref().and_then(duration_from_proto),
            healthy_threshold: hc.healthy_threshold,
            unhealthy_threshold: hc.unhealthy_threshold,
        }),
        outlier_detection: item
            .outlier_detection
            .as_ref()
            .map(|od| OutlierDetectionConfig {
                consecutive_5xx: od.consecutive_5xx,
                interval: od.interval.as_ref().and_then(duration_from_proto),
                base_ejection_time: od.base_ejection_time.as_ref().and_then(duration_from_proto),
                max_ejection_percent: od.max_ejection_percent,
            }),
    }
}

fn load_balancing_from_proto(item: proto::LoadBalancingPolicy) -> LoadBalancingPolicy {
    LoadBalancingPolicy {
        policy_type: load_balancing_type_from_proto(item.r#type()).to_string(),
        consistent_hash: item.consistent_hash.map(consistent_hash_from_proto),
        slow_start: item.slow_start.map(|ss| SlowStartConfig {
            window: ss.window.as_ref().and_then(duration_from_proto),
        }),
    }
}

fn consistent_hash_from_proto(item: proto::ConsistentHashPolicy) -> ConsistentHashPolicy {
    ConsistentHashPolicy {
        key_type: consistent_hash_key_type_from_proto(item.key_type()).to_string(),
        header_name: item.header_name,
    }
}

fn load_balancing_type_from_proto(item: proto::LoadBalancingPolicyType) -> &'static str {
    match item {
        proto::LoadBalancingPolicyType::LoadBalancingRoundRobin => "RoundRobin",
        proto::LoadBalancingPolicyType::LoadBalancingConsistentHash => "ConsistentHash",
        proto::LoadBalancingPolicyType::LoadBalancingLeastRequest => "LeastRequest",
        proto::LoadBalancingPolicyType::LoadBalancingRandom => "Random",
        proto::LoadBalancingPolicyType::LoadBalancingUnspecified => "",
    }
}

fn consistent_hash_key_type_from_proto(item: proto::ConsistentHashKeyType) -> &'static str {
    match item {
        proto::ConsistentHashKeyType::ConsistentHashSourceIp => "SourceIP",
        proto::ConsistentHashKeyType::ConsistentHashHeader => "Header",
        proto::ConsistentHashKeyType::ConsistentHashHostname => "Hostname",
        proto::ConsistentHashKeyType::ConsistentHashUnspecified => "",
    }
}

fn backend_tls_validation_from_proto(item: proto::BackendTlsValidation) -> BackendTlsValidation {
    BackendTlsValidation {
        hostname: item.hostname,
        use_system_ca_certificates: item.use_system_ca_certificates,
        ca_pems: item.ca_pems,
        subject_alt_names: item
            .subject_alt_names
            .into_iter()
            .map(backend_subject_alt_name_from_proto)
            .collect(),
        min_version: item.min_version,
        max_version: item.max_version,
    }
}

fn backend_subject_alt_name_from_proto(
    item: proto::BackendTlsSubjectAltName,
) -> BackendSubjectAltName {
    let kind = match item.r#type() {
        proto::BackendTlsSubjectAltNameType::BackendTlsSanHostname => "Hostname",
        proto::BackendTlsSubjectAltNameType::BackendTlsSanUri => "URI",
        proto::BackendTlsSubjectAltNameType::BackendTlsSanUnspecified => "",
    };

    BackendSubjectAltName {
        kind: kind.to_string(),
        value: item.value,
    }
}
