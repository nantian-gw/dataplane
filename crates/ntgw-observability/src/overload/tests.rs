use std::{sync::mpsc, thread, time::Duration};

use crate::{
    HttpAdmissionController, HttpAdmissionOptions, OverloadStats, TcpAdmissionController,
    TcpAdmissionOptions, UdpAdmissionController, UdpAdmissionOptions,
};

struct HeldHttpListenerInflightSnapshotRead<'a> {
    _guard: parking_lot::RwLockReadGuard<'a, std::collections::BTreeMap<String, u64>>,
}

fn hold_http_listener_inflight_snapshot_read<'a>(
    stats: &'a std::sync::Arc<super::OverloadStats>,
) -> HeldHttpListenerInflightSnapshotRead<'a> {
    HeldHttpListenerInflightSnapshotRead {
        _guard: stats.http_listener_inflight_current.read(),
    }
}

mod http_admission;
mod snapshot_nonblocking;
mod stream_admission;
