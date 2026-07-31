use thiserror::Error;

#[derive(Debug, Error)]
pub enum XdsError {
    #[error("xDS connection failed: {0}")]
    ConnectionFailed(#[source] anyhow::Error),

    #[error("xDS stream error: {0}")]
    StreamError(#[source] anyhow::Error),

    #[error("xDS TLS configuration error: {0}")]
    TlsConfig(#[source] anyhow::Error),

    #[error("xDS channel send error: {0}")]
    ChannelSend(#[source] anyhow::Error),
}

impl From<tonic::transport::Error> for XdsError {
    fn from(err: tonic::transport::Error) -> Self {
        XdsError::ConnectionFailed(anyhow::Error::from(err))
    }
}

impl From<anyhow::Error> for XdsError {
    fn from(err: anyhow::Error) -> Self {
        XdsError::StreamError(err)
    }
}

impl From<http::uri::InvalidUri> for XdsError {
    fn from(err: http::uri::InvalidUri) -> Self {
        XdsError::ConnectionFailed(anyhow::Error::from(err))
    }
}

impl From<tonic::Status> for XdsError {
    fn from(err: tonic::Status) -> Self {
        XdsError::StreamError(err.into())
    }
}

impl<T> From<tokio::sync::mpsc::error::SendError<T>> for XdsError {
    fn from(err: tokio::sync::mpsc::error::SendError<T>) -> Self {
        XdsError::ChannelSend(anyhow::anyhow!("{err}"))
    }
}
