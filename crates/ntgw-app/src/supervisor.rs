use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

use tokio::sync::watch;

#[derive(Clone, Debug)]
pub(crate) struct ShutdownCoordinator {
    requested: Arc<AtomicBool>,
    tx: watch::Sender<bool>,
}

impl ShutdownCoordinator {
    pub(crate) fn new() -> Self {
        let (tx, _rx) = watch::channel(false);
        Self {
            requested: Arc::new(AtomicBool::new(false)),
            tx,
        }
    }

    pub(crate) fn request_shutdown(&self) -> bool {
        if self.requested.swap(true, Ordering::AcqRel) {
            return false;
        }
        self.tx.send_replace(true);
        true
    }

    pub(crate) fn subscribe(&self) -> watch::Receiver<bool> {
        self.tx.subscribe()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ShutdownCause {
    pub(crate) graceful: bool,
    pub(crate) reason: String,
}

impl ShutdownCause {
    pub(crate) fn graceful(reason: impl Into<String>) -> Self {
        Self {
            graceful: true,
            reason: reason.into(),
        }
    }

    pub(crate) fn fatal(reason: impl Into<String>) -> Self {
        Self {
            graceful: false,
            reason: reason.into(),
        }
    }
}

pub(crate) async fn wait_for_shutdown(mut shutdown: watch::Receiver<bool>) {
    if *shutdown.borrow() {
        return;
    }

    let _ = shutdown.changed().await;
}

#[cfg(unix)]
pub(crate) async fn wait_for_termination_signal() -> ShutdownCause {
    use tokio::signal::unix::{SignalKind, signal};

    let mut sigterm = match signal(SignalKind::terminate()) {
        Ok(sigterm) => sigterm,
        Err(error) => {
            tracing::warn!(
                %error,
                "failed to install SIGTERM handler; falling back to ctrl-c only"
            );
            let _ = tokio::signal::ctrl_c().await;
            return ShutdownCause::graceful("signal: sigint");
        }
    };

    tokio::select! {
        _ = tokio::signal::ctrl_c() => ShutdownCause::graceful("signal: sigint"),
        _ = sigterm.recv() => ShutdownCause::graceful("signal: sigterm"),
    }
}

#[cfg(not(unix))]
pub(crate) async fn wait_for_termination_signal() -> ShutdownCause {
    let _ = tokio::signal::ctrl_c().await;
    ShutdownCause::graceful("signal: ctrl-c")
}

#[cfg(test)]
mod tests;
