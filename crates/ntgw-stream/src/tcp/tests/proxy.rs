use std::collections::BTreeMap;
use std::sync::Arc;

use anyhow::{Result, anyhow};
use ntgw_ir::{
    BackendCluster, BackendEndpoint, BackendRef, Listener, Snapshot, StreamRoute, StreamRule,
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    time::{Duration, timeout},
};

use super::*;

include!("proxy/plain_tcp.rs");
include!("proxy/tls_passthrough.rs");
include!("proxy/sni_priority.rs");
include!("proxy/half_close.rs");
