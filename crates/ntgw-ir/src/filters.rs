use serde::{Deserialize, Serialize};

use crate::BackendRef;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Filter {
    #[serde(rename = "type")]
    pub filter_type: String,
    pub header_modifier: Option<HeaderModifier>,
    pub cors: Option<CorsFilter>,
    pub request_redirect: Option<RequestRedirectFilter>,
    pub url_rewrite: Option<UrlRewriteFilter>,
    pub request_mirror: Option<RequestMirrorFilter>,
    pub external_auth: Option<ExternalAuthFilter>,
    pub extension_ref: Option<ExtensionFilter>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HeaderModifier {
    pub set: Vec<HeaderOperation>,
    pub add: Vec<HeaderOperation>,
    pub remove: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HeaderOperation {
    pub name: String,
    pub value: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CorsFilter {
    pub allow_origins: Vec<String>,
    pub allow_methods: Vec<String>,
    pub allow_headers: Vec<String>,
    pub expose_headers: Vec<String>,
    pub allow_credentials: bool,
    pub max_age: Option<u32>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RequestRedirectFilter {
    pub scheme: String,
    pub hostname: String,
    pub path: Option<PathModifier>,
    pub port: u32,
    pub status_code: u16,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct UrlRewriteFilter {
    pub hostname: String,
    pub path: Option<PathModifier>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PathModifier {
    pub modifier_type: String,
    pub replace_full_path: String,
    pub replace_prefix_match: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RequestMirrorFilter {
    pub backend_ref: BackendRef,
    pub percent: Option<u32>,
    pub fraction: Option<Fraction>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ExternalAuthFilter {
    pub protocol: String,
    pub backend_ref: BackendRef,
    pub http: ExternalHTTPAuthConfig,
    pub grpc: ExternalGRPCAuthConfig,
    pub forward_body_max_size: Option<u32>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ExternalHTTPAuthConfig {
    pub path: String,
    pub allowed_headers: Vec<String>,
    pub allowed_response_headers: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ExternalGRPCAuthConfig {
    pub allowed_headers: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ExtensionFilter {
    pub resolved: bool,
    pub message: String,
    pub extension_type: String,
    pub direct_response: Option<DirectResponseFilter>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DirectResponseFilter {
    pub status_code: u16,
    pub body: String,
    pub content_type: String,
    pub headers: Vec<HeaderOperation>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Fraction {
    pub numerator: u32,
    pub denominator: u32,
}

impl Default for Fraction {
    fn default() -> Self {
        Self {
            numerator: 0,
            denominator: 100,
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MatchedHttpPath {
    pub path: String,
    pub path_type: String,
}
