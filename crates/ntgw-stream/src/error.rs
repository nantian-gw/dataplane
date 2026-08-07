use thiserror::Error;

#[derive(Debug, Error)]
pub enum StreamError {
    #[error("TCP connection error: {0}")]
    TcpConnection(#[source] std::io::Error),

    #[error("UDP connection error: {0}")]
    UdpConnection(#[source] std::io::Error),

    #[error("stream dispatch error: {0}")]
    Dispatch(String),

    #[error("stream listener error: {0}")]
    Listener(String),
}

impl From<std::io::Error> for StreamError {
    fn from(err: std::io::Error) -> Self {
        StreamError::TcpConnection(err)
    }
}


impl From<std::net::AddrParseError> for StreamError {
    fn from(err: std::net::AddrParseError) -> Self {
        StreamError::Dispatch(err.to_string())
    }
}
