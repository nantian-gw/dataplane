use pingora::proxy::Session;

#[derive(Debug, Clone)]
pub struct DownstreamTlsInfo {
    pub server_name: String,
    pub client_certificate_present: bool,
}

pub(crate) fn downstream_tls_server_name(session: &Session) -> Option<String> {
    session
        .as_downstream()
        .digest()?
        .ssl_digest
        .as_ref()?
        .extension
        .get::<DownstreamTlsInfo>()
        .map(|info| info.server_name.as_str())
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

pub(crate) fn downstream_tls_client_certificate_present(session: &Session) -> bool {
    session
        .as_downstream()
        .digest()
        .and_then(|digest| digest.ssl_digest.as_ref())
        .and_then(|ssl| ssl.extension.get::<DownstreamTlsInfo>())
        .is_some_and(|info| info.client_certificate_present)
}
