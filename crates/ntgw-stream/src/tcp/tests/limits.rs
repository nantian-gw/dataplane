use anyhow::Result;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    time::{Duration, timeout},
};

use super::*;

include!("limits/connection_budget.rs");
include!("limits/idle_timeout.rs");
include!("limits/max_connection_age.rs");
