use thiserror::Error;

#[derive(Debug, Error)]
pub enum StreamError {
    #[error("TCP connection error: {0}")]
    TcpConnection(#[source] std::io::Error),

    #[error("UDP connection error: {0}")]
    UdpConnection(#[source] std::io::Error),

    #[error("stream dispatch error: {0}")]
    Dispatch(#[source] anyhow::Error),

    #[error("stream listener error: {0}")]
    Listener(#[source] anyhow::Error),
}

impl From<std::io::Error> for StreamError {
    fn from(err: std::io::Error) -> Self {
        StreamError::TcpConnection(err)
    }
}

impl From<anyhow::Error> for StreamError {
    fn from(err: anyhow::Error) -> Self {
        StreamError::Dispatch(err)
    }
}

impl From<std::net::AddrParseError> for StreamError {
    fn from(err: std::net::AddrParseError) -> Self {
        StreamError::Dispatch(anyhow::Error::from(err))
    }
}
