use std::sync::Arc;

use anyhow::Result;
use ntgw_ir::Listener;
use ntgw_observability::{
    AccessLogOptions, RuntimeStats, SharedTrafficStats, TcpAdmissionController,
    TcpAdmissionOptions, UdpAdmissionController, UdpAdmissionOptions,
};
use tokio::{net::TcpListener, sync::watch, time::Duration};

use crate::listener_plan::{ListenerPlan, PlannedListener, StreamProtocol};

use super::{ListenerSet, ReloadableRuntimeConfig, RuntimeOptions, run};

mod listener_replace;
mod reload_connections;
mod runtime_options;
mod unchanged_plan_apply;
