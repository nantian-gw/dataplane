use thiserror::Error;

#[derive(Debug, Error)]
pub enum SharedTlsError {
    #[error("TLS certificate error: {0}")]
    Certificate(#[source] anyhow::Error),

    #[error("TLS handshake error: {0}")]
    Handshake(#[source] anyhow::Error),

    #[error("TLS bind error: {0}")]
    Bind(#[source] anyhow::Error),

    #[error("TLS identity configuration error: {0}")]
    IdentityConfig(#[source] anyhow::Error),
}

impl From<anyhow::Error> for SharedTlsError {
    fn from(err: anyhow::Error) -> Self {
        SharedTlsError::Handshake(err)
    }
}

impl From<std::io::Error> for SharedTlsError {
    fn from(err: std::io::Error) -> Self {
        SharedTlsError::Bind(anyhow::Error::from(err))
    }
}

impl From<tokio::time::error::Elapsed> for SharedTlsError {
    fn from(err: tokio::time::error::Elapsed) -> Self {
        SharedTlsError::Handshake(anyhow::Error::from(err))
    }
}

impl From<Box<dyn std::error::Error + Send + Sync>> for SharedTlsError {
    fn from(err: Box<dyn std::error::Error + Send + Sync>) -> Self {
        SharedTlsError::Handshake(anyhow::Error::msg(err.to_string()))
    }
}

impl From<Box<pingora::Error>> for SharedTlsError {
    fn from(err: Box<pingora::Error>) -> Self {
        SharedTlsError::Handshake(anyhow::Error::msg(err.to_string()))
    }
}
