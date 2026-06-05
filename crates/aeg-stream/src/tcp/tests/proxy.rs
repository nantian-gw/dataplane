use std::collections::BTreeMap;

use aeg_ir::{
    BackendCluster, BackendEndpoint, BackendRef, Listener, Snapshot, StreamRoute, StreamRule,
};
use anyhow::{anyhow, Result};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    time::{timeout, Duration},
};

use super::*;

include!("proxy/plain_tcp.rs");
include!("proxy/tls_passthrough.rs");
include!("proxy/sni_priority.rs");
include!("proxy/half_close.rs");
