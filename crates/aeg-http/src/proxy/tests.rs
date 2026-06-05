mod backend_client_cert;
mod backend_protocol;
mod backend_tls_validation;
mod connection_header;
mod context;
mod fast_path;
mod mesh_fallback;
mod retry;
mod selection;

pub(super) const TEST_CLIENT_CERT_PEM: &str = include_str!("../../../../testdata/tls/client.crt");
pub(super) const TEST_CLIENT_KEY_PEM: &str = include_str!("../../../../testdata/tls/client.key");
pub(super) const TEST_SERVER_SAN_CERT_PEM: &str =
    include_str!("../../../../testdata/backendtls/server-san.crt");
pub(super) const TEST_SERVER_SAN_KEY_PEM: &str =
    include_str!("../../../../testdata/backendtls/server-san.key");
