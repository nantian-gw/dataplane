use super::values::{
    list_value, optional_bool, optional_string, optional_string_map, optional_u32, string_value,
    struct_value,
};
use super::*;

pub(super) fn filter_from_proto(item: proto::Filter) -> Filter {
    let filter_type = item.r#type;
    let config = item.config;
    Filter {
        header_modifier: match filter_type.as_str() {
            "RequestHeaderModifier" | "ResponseHeaderModifier" => {
                header_modifier_from_proto(config.clone())
            }
            _ => None,
        },
        cors: match filter_type.as_str() {
            "CORS" => cors_from_proto(config.clone()),
            _ => None,
        },
        request_redirect: match filter_type.as_str() {
            "RequestRedirect" => request_redirect_from_proto(config.clone()),
            _ => None,
        },
        url_rewrite: match filter_type.as_str() {
            "URLRewrite" => url_rewrite_from_proto(config.clone()),
            _ => None,
        },
        request_mirror: match filter_type.as_str() {
            "RequestMirror" => request_mirror_from_proto(config.clone()),
            _ => None,
        },
        external_auth: match filter_type.as_str() {
            "ExternalAuth" => external_auth_from_proto(config.clone()),
            _ => None,
        },
        extension_ref: match filter_type.as_str() {
            "ExtensionRef" => extension_filter_from_proto(config),
            _ => None,
        },
        filter_type,
    }
}

fn cors_from_proto(config: Option<Struct>) -> Option<CorsFilter> {
    let config = config?;
    Some(CorsFilter {
        allow_origins: string_list(config.fields.get("allowOrigins")),
        allow_methods: string_list(config.fields.get("allowMethods")),
        allow_headers: string_list(config.fields.get("allowHeaders")),
        expose_headers: string_list(config.fields.get("exposeHeaders")),
        allow_credentials: optional_bool(config.fields.get("allowCredentials")).unwrap_or(false),
        max_age: optional_u32(config.fields.get("maxAge")),
    })
}

fn header_modifier_from_proto(config: Option<Struct>) -> Option<HeaderModifier> {
    let config = config?;
    let modifier = HeaderModifier {
        set: header_operations(config.fields.get("set")),
        add: header_operations(config.fields.get("add")),
        remove: string_list(config.fields.get("remove")),
    };

    if modifier.set.is_empty() && modifier.add.is_empty() && modifier.remove.is_empty() {
        None
    } else {
        Some(modifier)
    }
}

fn header_operations(value: Option<&Value>) -> Vec<HeaderOperation> {
    list_value(value)
        .map(|list| {
            list.values
                .iter()
                .filter_map(header_operation_from_value)
                .collect()
        })
        .unwrap_or_default()
}

fn header_operation_from_value(value: &Value) -> Option<HeaderOperation> {
    let fields = struct_value(value)?;
    Some(HeaderOperation {
        name: string_value(fields.fields.get("name")?)?,
        value: string_value(fields.fields.get("value")?)?,
    })
}

fn string_list(value: Option<&Value>) -> Vec<String> {
    list_value(value)
        .map(|list| list.values.iter().filter_map(string_value).collect())
        .unwrap_or_default()
}

fn request_redirect_from_proto(config: Option<Struct>) -> Option<RequestRedirectFilter> {
    let config = config?;
    Some(RequestRedirectFilter {
        scheme: optional_string(config.fields.get("scheme")).unwrap_or_default(),
        hostname: optional_string(config.fields.get("hostname")).unwrap_or_default(),
        path: path_modifier_from_value(config.fields.get("path")),
        port: optional_u32(config.fields.get("port")).unwrap_or_default(),
        status_code: optional_u32(config.fields.get("statusCode"))
            .map(|item| item as u16)
            .unwrap_or(302),
    })
}

fn url_rewrite_from_proto(config: Option<Struct>) -> Option<UrlRewriteFilter> {
    let config = config?;
    Some(UrlRewriteFilter {
        hostname: optional_string(config.fields.get("hostname")).unwrap_or_default(),
        path: path_modifier_from_value(config.fields.get("path")),
    })
}

fn request_mirror_from_proto(config: Option<Struct>) -> Option<RequestMirrorFilter> {
    let config = config?;
    Some(RequestMirrorFilter {
        backend_ref: backend_ref_from_value(config.fields.get("backendRef"))?,
        percent: optional_u32(config.fields.get("percent")),
        fraction: fraction_from_value(config.fields.get("fraction")),
    })
}

fn external_auth_from_proto(config: Option<Struct>) -> Option<ExternalAuthFilter> {
    let config = config?;
    Some(ExternalAuthFilter {
        protocol: optional_string(config.fields.get("protocol")).unwrap_or_default(),
        backend_ref: backend_ref_from_value(config.fields.get("backendRef"))?,
        http: external_http_auth_from_value(config.fields.get("http")).unwrap_or_default(),
        grpc: external_grpc_auth_from_value(config.fields.get("grpc")).unwrap_or_default(),
        forward_body_max_size: optional_u32(config.fields.get("forwardBodyMaxSize")),
    })
}

fn external_http_auth_from_value(value: Option<&Value>) -> Option<ExternalHTTPAuthConfig> {
    let item = struct_value(value?)?;
    Some(ExternalHTTPAuthConfig {
        path: optional_string(item.fields.get("path")).unwrap_or_default(),
        allowed_headers: string_list(item.fields.get("allowedHeaders")),
        allowed_response_headers: string_list(item.fields.get("allowedResponseHeaders")),
    })
}

fn external_grpc_auth_from_value(value: Option<&Value>) -> Option<ExternalGRPCAuthConfig> {
    let item = struct_value(value?)?;
    Some(ExternalGRPCAuthConfig {
        allowed_headers: string_list(item.fields.get("allowedHeaders")),
    })
}

fn extension_filter_from_proto(config: Option<Struct>) -> Option<ExtensionFilter> {
    let config = config?;
    let extension_type = optional_string(config.fields.get("extensionType")).unwrap_or_default();
    Some(ExtensionFilter {
        resolved: optional_bool(config.fields.get("resolved")).unwrap_or(false),
        message: optional_string(config.fields.get("message")).unwrap_or_default(),
        direct_response: match extension_type.as_str() {
            "DirectResponse" => config
                .fields
                .get("directResponse")
                .or_else(|| config.fields.get("direct_response"))
                .and_then(|value| direct_response_from_proto(Some(value)))
                .or_else(|| direct_response_from_struct(&config)),
            _ => None,
        },
        extension_type,
    })
}

fn direct_response_from_proto(value: Option<&Value>) -> Option<DirectResponseFilter> {
    let config = struct_value(value?)?;
    direct_response_from_struct(config)
}

fn direct_response_from_struct(config: &Struct) -> Option<DirectResponseFilter> {
    Some(DirectResponseFilter {
        status_code: optional_u32(config.fields.get("statusCode"))
            .map(|item| item as u16)
            .unwrap_or(500),
        body: optional_string(config.fields.get("body")).unwrap_or_default(),
        content_type: optional_string(config.fields.get("contentType")).unwrap_or_default(),
        headers: header_operations(config.fields.get("headers")),
    })
}

fn path_modifier_from_value(value: Option<&Value>) -> Option<PathModifier> {
    let item = struct_value(value?)?;
    Some(PathModifier {
        modifier_type: optional_string(item.fields.get("type")).unwrap_or_default(),
        replace_full_path: optional_string(item.fields.get("replaceFullPath")).unwrap_or_default(),
        replace_prefix_match: optional_string(item.fields.get("replacePrefixMatch"))
            .unwrap_or_default(),
    })
}

fn backend_ref_from_value(value: Option<&Value>) -> Option<BackendRef> {
    let item = struct_value(value?)?;
    Some(BackendRef {
        group: optional_string(item.fields.get("group")).unwrap_or_default(),
        kind: optional_string(item.fields.get("kind")).unwrap_or_default(),
        namespace: optional_string(item.fields.get("namespace")).unwrap_or_default(),
        name: optional_string(item.fields.get("name")).unwrap_or_default(),
        port: optional_u32(item.fields.get("port")).unwrap_or_default(),
        weight: optional_u32(item.fields.get("weight")).unwrap_or(1),
        metadata: optional_string_map(item.fields.get("metadata")).unwrap_or_default(),
        filters: optional_filter_list(item.fields.get("filters")).unwrap_or_default(),
    })
}

fn optional_filter_list(value: Option<&Value>) -> Option<Vec<Filter>> {
    Some(
        list_value(value)?
            .values
            .iter()
            .filter_map(struct_value)
            .filter_map(filter_from_struct_value)
            .collect(),
    )
}

fn filter_from_struct_value(item: &Struct) -> Option<Filter> {
    Some(filter_from_proto(proto::Filter {
        r#type: optional_string(item.fields.get("type"))?,
        config: item.fields.get("config").and_then(struct_value).cloned(),
    }))
}

fn fraction_from_value(value: Option<&Value>) -> Option<Fraction> {
    let item = struct_value(value?)?;
    Some(Fraction {
        numerator: optional_u32(item.fields.get("numerator")).unwrap_or_default(),
        denominator: optional_u32(item.fields.get("denominator"))
            .unwrap_or(100)
            .max(1),
    })
}
