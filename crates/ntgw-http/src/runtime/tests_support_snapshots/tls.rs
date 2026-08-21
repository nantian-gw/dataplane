fn example_secret_material() -> SecretMaterial {
    SecretMaterial {
        htpasswd: String::new(),
        oidc_client_secret: String::new(),
        namespace: "default".to_string(),
        name: "example-cert".to_string(),
        cert_pem: VALID_SERVER_CERT_PEM.to_string(),
        key_pem: VALID_SERVER_KEY_PEM.to_string(),
    }
}

fn single_tls_material(
    secret_ref: &str,
    cert_pem: &str,
    key_pem: &str,
    client_ca_bundle_pem: Option<&str>,
) -> super::TlsMaterial {
    super::TlsMaterial {
        identities: vec![super::TlsIdentity {
            secret_ref: secret_ref.to_string(),
            cert_pem: cert_pem.to_string(),
            key_pem: key_pem.to_string(),
            match_names: Vec::new(),
        }],
        min_version: "1.2".to_string(),
        max_version: "1.3".to_string(),
        client_ca_bundle_pem: client_ca_bundle_pem.map(str::to_string),
        frontend_validation_mode: None,
    }
}
