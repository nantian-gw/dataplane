use std::{collections::BTreeMap, time::Duration};

use ntgw_proto::gateway::control::v1 as proto;
use prost_types::{Duration as ProtoDuration, ListValue, Struct, Value, value::Kind};

use crate::{
    AIServiceAuthConfig, AIServiceConfig, BackendCluster, BackendEndpoint, BackendPolicy,
    BackendRef, BackendSubjectAltName, BackendTlsValidation, BasicAuthConfig, ConsistentHashPolicy,
    CorsFilter, DirectResponseFilter, ExtensionFilter, ExternalAuthConfig, ExternalAuthFilter,
    ExternalGRPCAuthConfig, ExternalGrpcAuth, ExternalHTTPAuthConfig, ExternalHttpAuth, Filter,
    Fraction, GrpcMatch, GrpcRoute, GrpcRule, HeaderMatch, HeaderModifier, HeaderOperation,
    HttpMatch, HttpRoute, HttpRule, JwtAuthConfig, Listener, LoadBalancingPolicy, OidcAuthConfig,
    ParentRef, PathModifier, QueryMatch, RateLimitRule, RequestMirrorFilter, RequestRedirectFilter,
    RetryPolicy, RouteTimeouts, SecretMaterial, SecurityAuthNConfig, SecurityAuthZConfig,
    SecurityCorsConfig, SecurityIpConfig, SecurityPolicyConfig, SessionPersistence, Snapshot,
    StreamMatch, StreamRoute, StreamRule, TlsConfig, TokenPolicyConfig, UrlRewriteFilter,
    WasmPluginConfig, WasmSandboxConfig, Workload,
};

mod backend;
mod filters;
mod routes;
mod values;

use self::backend::{backend_from_proto, backend_policy_from_proto, workloads_from_extensions};
use self::routes::{
    backend_tls_from_proto, grpc_rule_from_proto, http_rule_from_proto, parent_ref_from_proto,
    stream_rule_from_proto, tls_from_proto,
};

impl Snapshot {
    pub fn from_proto_without_runtime_indexes(value: proto::ConfigSnapshot) -> Self {
        let workloads = workloads_from_extensions(value.extensions.as_ref());
        let mut backends = Vec::with_capacity(value.backends.len());
        let mut backend_policies = BTreeMap::new();
        for item in value.backends {
            let backend_name = format!("{}/{}", item.namespace, item.name);
            let policy = backend_policy_from_proto(&item);
            if policy.connect_timeout.is_some()
                || policy.request_timeout.is_some()
                || policy.tls_validation.is_some()
                || policy.session_persistence.is_some()
                || policy.load_balancing.is_some()
                || policy.health_check.is_some()
                || policy.outlier_detection.is_some()
            {
                backend_policies.insert(backend_name, policy);
            }
            backends.push(backend_from_proto(item));
        }

        Self {
            id: value.id,
            listeners: value
                .listeners
                .into_iter()
                .map(|item| {
                    let protocol = item.protocol().as_str_name().to_string();
                    Listener {
                        name: item.name,
                        address: item.address,
                        addresses: item.addresses,
                        port: item.port,
                        protocol,
                        hostnames: item.hostnames,
                        attached_routes: item.attached_routes,
                        tls: item.tls.map(tls_from_proto),
                        backend_tls: item.backend_tls.map(backend_tls_from_proto),
                        metadata: item.metadata.into_iter().collect(),
                    }
                })
                .collect(),
            http_routes: value
                .http_routes
                .into_iter()
                .map(|item| HttpRoute {
                    name: item.name,
                    namespace: item.namespace,
                    hostnames: item.hostnames,
                    parent_refs: item
                        .parent_refs
                        .into_iter()
                        .map(parent_ref_from_proto)
                        .collect(),
                    rules: item.rules.into_iter().map(http_rule_from_proto).collect(),
                    labels: item.labels.into_iter().collect(),
                    annotations: item.annotations.into_iter().collect(),
                })
                .collect(),
            grpc_routes: value
                .grpc_routes
                .into_iter()
                .map(|item| GrpcRoute {
                    name: item.name,
                    namespace: item.namespace,
                    hostnames: item.hostnames,
                    parent_refs: item
                        .parent_refs
                        .into_iter()
                        .map(parent_ref_from_proto)
                        .collect(),
                    rules: item.rules.into_iter().map(grpc_rule_from_proto).collect(),
                    labels: item.labels.into_iter().collect(),
                    annotations: item.annotations.into_iter().collect(),
                })
                .collect(),
            stream_routes: value
                .stream_routes
                .into_iter()
                .map(|item| {
                    let kind = item.kind().as_str_name().to_string();
                    StreamRoute {
                        name: item.name,
                        namespace: item.namespace,
                        kind,
                        parent_refs: item
                            .parent_refs
                            .into_iter()
                            .map(parent_ref_from_proto)
                            .collect(),
                        rules: item.rules.into_iter().map(stream_rule_from_proto).collect(),
                        labels: item.labels.into_iter().collect(),
                        annotations: item.annotations.into_iter().collect(),
                    }
                })
                .collect(),
            backends,
            backend_policies,
            secrets: value
                .secrets
                .into_iter()
                .map(|item| SecretMaterial {
                    namespace: item.namespace,
                    name: item.name,
                    cert_pem: item.cert_pem,
                    key_pem: item.key_pem,
                    htpasswd: item.htpasswd,
                    oidc_client_secret: item.oidc_client_secret,
                })
                .collect(),
            workloads,
            selection_state: Default::default(),
            ..Self::default()
        }
    }
}

pub(super) fn security_policy_from_proto(
    proto: proto::SecurityPolicyConfig,
) -> SecurityPolicyConfig {
    let mut out = SecurityPolicyConfig::default();
    if let Some(authn) = proto.authn {
        let mut a = SecurityAuthNConfig::default();
        if let Some(jwt) = authn.jwt
            && let Some(issuer) = jwt.issuers.into_iter().next()
        {
            a.jwt = Some(JwtAuthConfig {
                issuer: issuer.issuer,
                jwks_url: issuer.jwks_url,
                audience: issuer.audience,
                header_name: issuer.header_name,
                token_prefix: issuer.token_prefix,
                claims_to_headers: issuer.claims_to_headers.into_iter().collect(),
                cache_ttl_secs: issuer.cache_ttl_secs,
            });
        }
        if let Some(oidc) = authn.oidc {
            a.oidc = Some(OidcAuthConfig {
                provider_authorization_url: oidc.provider_authorization_url,
                provider_token_url: oidc.provider_token_url,
                provider_jwks_url: oidc.provider_jwks_url,
                provider_userinfo_url: oidc.provider_userinfo_url,
                client_id: oidc.client_id,
                client_secret_ref: oidc.client_secret_ref,
                callback_path: oidc.callback_path,
                scopes: oidc.scopes,
                redirect_url: oidc.redirect_url,
                session_signing_key_ref: oidc.session_signing_key_ref,
                session_cookie_name: oidc.session_cookie_name,
                session_ttl_secs: oidc.session_ttl_secs,
            });
        }
        if let Some(ba) = authn.basic_auth {
            a.basic_auth = Some(BasicAuthConfig {
                htpasswd_ref: ba.htpasswd_ref,
                bcrypt: ba.bcrypt,
                realm: ba.realm,
            });
        }
        out.authn = Some(a);
    }
    if let Some(authz) = proto.authz
        && let Some(ext) = authz.external
    {
        out.authz = Some(SecurityAuthZConfig {
            external: Some(ExternalAuthConfig {
                protocol: match ext.protocol() {
                    proto::ExternalAuthTransport::Http => "HTTP".to_string(),
                    proto::ExternalAuthTransport::Grpc => "GRPC".to_string(),
                    _ => String::new(),
                },
                backend_ref: ext.backend_ref.map(|br| BackendRef {
                    namespace: br.namespace,
                    name: br.name,
                    port: br.port,
                    ..Default::default()
                }),
                http: ext.http.map(|h| ExternalHttpAuth {
                    path_prefix: h.path_prefix,
                    headers_to_add: h.headers_to_add,
                }),
                grpc: ext.grpc.map(|g| ExternalGrpcAuth {
                    grpc_service: g.grpc_service,
                }),
                forward_body_max_size: ext.forward_body_max_size,
            }),
        });
    }
    if let Some(cors) = proto.cors {
        out.cors = Some(SecurityCorsConfig {
            allow_origins: cors.allow_origins,
            allow_methods: cors.allow_methods,
            allow_headers: cors.allow_headers,
            expose_headers: cors.expose_headers,
            allow_credentials: cors.allow_credentials,
            max_age: cors.max_age,
        });
    }
    out.rate_limit = proto
        .rate_limit
        .into_iter()
        .map(|r| {
            let key_type = r.key_type.clone();
            RateLimitRule {
                scope: match r.scope() {
                    proto::RateLimitScope::Global => "global".to_string(),
                    proto::RateLimitScope::Listener => "listener".to_string(),
                    proto::RateLimitScope::Route => "route".to_string(),
                    proto::RateLimitScope::Backend => "backend".to_string(),
                    _ => String::new(),
                },
                requests_per_second: r.requests_per_second,
                burst: r.burst,
                key_type,
                on_limit: match r.on_limit() {
                    proto::RateLimitAction::Reject => "reject".to_string(),
                    proto::RateLimitAction::Queue => "queue".to_string(),
                    _ => String::new(),
                },
                key_header_name: r.key_header_name,
            }
        })
        .collect();
    if let Some(ip) = proto.ip {
        out.ip = Some(SecurityIpConfig {
            allow_cidrs: ip.allow_cidrs,
            deny_cidrs: ip.deny_cidrs,
        });
    }
    out
}

impl From<proto::ConfigSnapshot> for Snapshot {
    fn from(value: proto::ConfigSnapshot) -> Self {
        let mut snapshot = Self::from_proto_without_runtime_indexes(value);
        snapshot.rebuild_runtime_indexes();
        snapshot
    }
}
