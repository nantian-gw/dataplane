use super::*;

#[test]
fn ensure_supported_filters_rejects_unknown_types() {
    let result = ensure_supported_filters(&[Filter {
        filter_type: "Custom".to_string(),
        ..Filter::default()
    }]);

    assert!(result.is_err());
}

#[test]
fn ensure_supported_filters_allows_known_types() {
    let result = ensure_supported_filters(&[
        Filter {
            filter_type: "RequestHeaderModifier".to_string(),
            ..Filter::default()
        },
        Filter {
            filter_type: "RequestRedirect".to_string(),
            ..Filter::default()
        },
        Filter {
            filter_type: "CORS".to_string(),
            ..Filter::default()
        },
        Filter {
            filter_type: "RequestMirror".to_string(),
            ..Filter::default()
        },
        Filter {
            filter_type: "ExternalAuth".to_string(),
            external_auth: Some(ntgw_ir::ExternalAuthFilter {
                protocol: "HTTP".to_string().into(),
                ..ntgw_ir::ExternalAuthFilter::default()
            }),
            ..Filter::default()
        },
        Filter {
            filter_type: "ExternalAuth".to_string(),
            external_auth: Some(ntgw_ir::ExternalAuthFilter {
                protocol: "GRPC".to_string().into(),
                ..ntgw_ir::ExternalAuthFilter::default()
            }),
            ..Filter::default()
        },
    ]);

    assert!(result.is_ok());
}

#[test]
fn ensure_supported_filters_allows_external_auth_forward_body() {
    let result = ensure_supported_filters(&[Filter {
        filter_type: "ExternalAuth".to_string(),
        external_auth: Some(ntgw_ir::ExternalAuthFilter {
            protocol: "HTTP".to_string().into(),
            forward_body_max_size: Some(1),
            ..ntgw_ir::ExternalAuthFilter::default()
        }),
        ..Filter::default()
    }]);

    assert!(result.is_ok());
}
