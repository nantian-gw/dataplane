use super::*;

#[test]
fn backend_certificate_matches_any_configured_subject_alt_name() {
    let certificates =
        pingora::tls::x509::X509::stack_from_pem(TEST_SERVER_SAN_CERT_PEM.as_bytes())
            .expect("parse test server certificate");
    let certificate = certificates.first().expect("leaf certificate");

    let matched = backend_certificate_matches_subject_alt_names(
        certificate,
        &[
            aeg_ir::BackendSubjectAltName {
                kind: "Hostname".to_string(),
                value: "missing.backend.svc".to_string(),
            },
            aeg_ir::BackendSubjectAltName {
                kind: "URI".to_string(),
                value: "spiffe://cluster.local/ns/default/sa/orders".to_string(),
            },
        ],
    )
    .expect("subjectAltNames evaluation");
    assert!(matched);

    let unmatched = backend_certificate_matches_subject_alt_names(
        certificate,
        &[
            aeg_ir::BackendSubjectAltName {
                kind: "Hostname".to_string(),
                value: "missing.backend.svc".to_string(),
            },
            aeg_ir::BackendSubjectAltName {
                kind: "URI".to_string(),
                value: "spiffe://cluster.local/ns/default/sa/payments".to_string(),
            },
        ],
    )
    .expect("subjectAltNames evaluation");
    assert!(!unmatched);
}
