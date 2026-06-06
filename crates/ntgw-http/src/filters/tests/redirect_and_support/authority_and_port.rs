use super::*;

#[test]
fn redirect_authority_skips_default_ports() {
    assert_eq!(redirect_authority("http", "example.com", 80), "example.com");
    assert_eq!(
        redirect_authority("https", "example.com", 443),
        "example.com"
    );
    assert_eq!(
        redirect_authority("https", "example.com", 8443),
        "example.com:8443"
    );
}

#[test]
fn redirect_helpers_treat_scheme_case_insensitively() {
    assert_eq!(
        redirect_port("HTTPS", "http", 8080, &RequestRedirectFilter::default(),),
        443
    );
    assert_eq!(
        redirect_authority("HTTPS", "example.com", 443),
        "example.com"
    );
}

#[test]
fn redirect_port_preserves_non_default_listener_port_for_same_scheme() {
    assert_eq!(
        redirect_port("http", "http", 8080, &RequestRedirectFilter::default(),),
        8080
    );
    assert_eq!(
        redirect_port("https", "https", 8443, &RequestRedirectFilter::default(),),
        8443
    );
}

#[test]
fn redirect_port_uses_target_scheme_default_when_scheme_changes() {
    assert_eq!(
        redirect_port("https", "http", 8080, &RequestRedirectFilter::default(),),
        443
    );
    assert_eq!(
        redirect_port("http", "https", 8443, &RequestRedirectFilter::default(),),
        80
    );
}

#[test]
fn redirect_port_prefers_explicit_redirect_port() {
    assert_eq!(
        redirect_port(
            "https",
            "http",
            8080,
            &RequestRedirectFilter {
                port: 9443,
                ..RequestRedirectFilter::default()
            },
        ),
        9443
    );
}
