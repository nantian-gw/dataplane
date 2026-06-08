use ntgw_ir::{DirectResponseFilter, ExtensionFilter, Filter, HeaderOperation};

use super::{direct_response_filter, extension_filters_supported};

#[test]
fn accepts_resolved_direct_response_extensions() {
    let filters = vec![Filter {
        filter_type: "ExtensionRef".to_string(),
        extension_ref: Some(ExtensionFilter {
            resolved: true,
            extension_type: "DirectResponse".to_string(),
            direct_response: Some(DirectResponseFilter {
                status_code: 503,
                body: "maintenance".to_string(),
                content_type: "text/plain".to_string(),
                headers: vec![HeaderOperation {
                    name: "retry-after".to_string(),
                    value: "60".to_string(),
                }],
            }),
            ..ExtensionFilter::default()
        }),
        ..Filter::default()
    }];

    assert!(extension_filters_supported(&filters));
    assert_eq!(
        direct_response_filter(&filters)
            .expect("direct response")
            .status_code,
        503
    );
}

#[test]
fn rejects_unresolved_extensions() {
    let filters = vec![Filter {
        filter_type: "ExtensionRef".to_string(),
        extension_ref: Some(ExtensionFilter {
            resolved: false,
            message: "missing".to_string(),
            ..ExtensionFilter::default()
        }),
        ..Filter::default()
    }];

    assert!(!extension_filters_supported(&filters));
}
