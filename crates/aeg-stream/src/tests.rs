use std::sync::Arc;

use aeg_ir::Listener;
use aeg_observability::{
    AccessLogOptions, RuntimeStats, SharedTrafficStats, TcpAdmissionController,
    TcpAdmissionOptions, UdpAdmissionController, UdpAdmissionOptions,
};
use anyhow::Result;
use tokio::{net::TcpListener, sync::watch, time::Duration};

use crate::listener_plan::{ListenerPlan, PlannedListener, StreamProtocol};

use super::{run, ListenerSet, ReloadableRuntimeConfig, RuntimeOptions};

mod listener_replace;
mod reload_connections;
mod runtime_options;
mod unchanged_plan_apply;
