use thiserror::Error;

#[derive(Debug, Error)]
pub enum SharedTlsError {
    #[error("TLS certificate error: {0}")]
    Certificate(String),

    #[error("TLS handshake error: {0}")]
    Handshake(String),

    #[error("TLS bind error: {0}")]
    Bind(String),

    #[error("TLS identity configuration error: {0}")]
    IdentityConfig(String),
}

impl From<std::io::Error> for SharedTlsError {
    fn from(err: std::io::Error) -> Self {
        SharedTlsError::Bind(err.to_string())
    }
}

impl From<tokio::time::error::Elapsed> for SharedTlsError {
    fn from(err: tokio::time::error::Elapsed) -> Self {
        SharedTlsError::Handshake(err.to_string())
    }
}

impl From<Box<dyn std::error::Error + Send + Sync>> for SharedTlsError {
    fn from(err: Box<dyn std::error::Error + Send + Sync>) -> Self {
        SharedTlsError::Handshake(err.to_string())
    }
}

impl From<Box<pingora::Error>> for SharedTlsError {
    fn from(err: Box<pingora::Error>) -> Self {
        SharedTlsError::Handshake(err.to_string())
    }
}
