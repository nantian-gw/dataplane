use thiserror::Error;

#[derive(Debug, Error)]
pub enum XdsError {
    #[error("xDS connection failed: {0}")]
    ConnectionFailed(String),

    #[error("xDS stream error: {0}")]
    StreamError(String),

    #[error("xDS TLS configuration error: {0}")]
    TlsConfig(String),

    #[error("xDS channel send error: {0}")]
    ChannelSend(String),
}

impl From<tonic::transport::Error> for XdsError {
    fn from(err: tonic::transport::Error) -> Self {
        XdsError::ConnectionFailed(err.to_string())
    }
}


impl From<http::uri::InvalidUri> for XdsError {
    fn from(err: http::uri::InvalidUri) -> Self {
        XdsError::ConnectionFailed(err.to_string())
    }
}

impl From<tonic::Status> for XdsError {
    fn from(err: tonic::Status) -> Self {
        XdsError::StreamError(err.to_string())
    }
}

impl<T> From<tokio::sync::mpsc::error::SendError<T>> for XdsError {
    fn from(err: tokio::sync::mpsc::error::SendError<T>) -> Self {
        XdsError::ChannelSend(err.to_string())
    }
}
