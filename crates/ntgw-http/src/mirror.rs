use std::{
    fmt,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
    time::Duration,
};

use bytes::Bytes;
use ntgw_ir::SelectedBackend;
use pingora::{
    protocols::http::HttpTask,
    proxy::{
        subrequest::{BodyMode, Ctx as SubrequestCtx},
        Session,
    },
};
use tokio::sync::{mpsc, oneshot, OwnedSemaphorePermit, Semaphore};
use tracing::warn;

const MAX_ACTIVE_REQUEST_MIRRORS: usize = 1_024;
const REQUEST_MIRROR_DROP_WARN_INTERVAL: u64 = 256;
const REQUEST_MIRROR_FAILURE_WARN_INTERVAL: u64 = 256;
const REQUEST_MIRROR_SKIP_WARN_INTERVAL: u64 = 256;
const REQUEST_MIRROR_WAIT_TIMEOUT: Duration = Duration::from_millis(100);
const REQUEST_MIRROR_WAIT_WARN_INTERVAL: u64 = 256;
static REQUEST_MIRROR_LIMIT: std::sync::OnceLock<usize> = std::sync::OnceLock::new();
static REQUEST_MIRROR_SEMAPHORE: std::sync::OnceLock<Arc<Semaphore>> = std::sync::OnceLock::new();
static REQUEST_MIRROR_DROPPED_TASKS: AtomicU64 = AtomicU64::new(0);
static REQUEST_MIRROR_FAILED_TASKS: AtomicU64 = AtomicU64::new(0);
static REQUEST_MIRROR_PROXY_ERRORS: AtomicU64 = AtomicU64::new(0);
static REQUEST_MIRROR_SKIPPED_NO_SPAWNER: AtomicU64 = AtomicU64::new(0);
static REQUEST_MIRROR_SKIPPED_BUDGET: AtomicU64 = AtomicU64::new(0);
static REQUEST_MIRROR_WAIT_TIMEOUTS: AtomicU64 = AtomicU64::new(0);
static REQUEST_MIRROR_WAIT_JOIN_ERRORS: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Clone)]
struct MirrorSubrequest {
    selected_backend: SelectedBackend,
}

pub(crate) struct MirrorRequestSession {
    tx: mpsc::Sender<HttpTask>,
    forwards_body: bool,
    backend_name: String,
    completion: Option<tokio::task::JoinHandle<()>>,
}

impl fmt::Debug for MirrorRequestSession {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MirrorRequestSession")
            .finish_non_exhaustive()
    }
}

pub(crate) fn is_mirror_subrequest(session: &Session) -> bool {
    mirror_subrequest(session).is_some()
}

pub(crate) fn selected_backend_from_subrequest(session: &Session) -> Option<&SelectedBackend> {
    mirror_subrequest(session).map(|item| &item.selected_backend)
}

pub fn configure_request_mirror_budget(limit: usize) {
    let limit = limit.max(1);
    if REQUEST_MIRROR_LIMIT.set(limit).is_err() {
        return;
    }

    if let Some(semaphore) = REQUEST_MIRROR_SEMAPHORE.get() {
        let current = semaphore.available_permits();
        if current < limit {
            semaphore.add_permits(limit - current);
        } else if current > limit {
            semaphore.forget_permits(current - limit);
        }
    }
}

pub(crate) fn spawn_request_mirror(
    session: &Session,
    selected_backend: SelectedBackend,
    request_has_body: bool,
) -> Option<MirrorRequestSession> {
    let Some(spawner) = session.subrequest_spawner.as_ref() else {
        let skipped = REQUEST_MIRROR_SKIPPED_NO_SPAWNER.fetch_add(1, Ordering::Relaxed) + 1;
        if skipped == 1 || skipped.is_multiple_of(REQUEST_MIRROR_SKIP_WARN_INTERVAL) {
            warn!(
                skipped,
                "request mirror skipped because subrequest spawner is unavailable"
            );
        }
        return None;
    };
    let permit = match request_mirror_semaphore().clone().try_acquire_owned() {
        Ok(permit) => permit,
        Err(_) => {
            let skipped = REQUEST_MIRROR_SKIPPED_BUDGET.fetch_add(1, Ordering::Relaxed) + 1;
            if skipped == 1 || skipped.is_multiple_of(REQUEST_MIRROR_SKIP_WARN_INTERVAL) {
                warn!(
                    limit = request_mirror_budget_limit(),
                    backend = %selected_backend.backend_name,
                    skipped,
                    "request mirror skipped because the concurrency budget is exhausted"
                );
            }
            return None;
        }
    };

    let body_mode = if request_has_body {
        BodyMode::ExpectBody
    } else {
        BodyMode::NoBody
    };
    let mirrored_backend_name = selected_backend.backend_name.clone();
    let subrequest_ctx = SubrequestCtx::builder()
        .body_mode(body_mode)
        .user_ctx(Box::new(MirrorSubrequest { selected_backend }))
        .build();
    let (prepared, handle) = spawner.create_subrequest(session.as_ref(), subrequest_ctx);

    tokio::spawn(async move {
        prepared.run().await;
    });

    let completion = tokio::spawn(drain_mirror_response(
        handle.rx,
        handle.subreq_proxy_error,
        permit,
    ));

    Some(MirrorRequestSession {
        tx: handle.tx.clone(),
        forwards_body: request_has_body,
        backend_name: mirrored_backend_name,
        completion: Some(completion),
    })
}

pub(crate) fn spawn_request_mirrors(
    session: &Session,
    selected_backends: Vec<SelectedBackend>,
    request_has_body: bool,
) -> Vec<MirrorRequestSession> {
    let mut active = Vec::with_capacity(selected_backends.len());
    for selected_backend in selected_backends {
        if let Some(mirror) = spawn_request_mirror(session, selected_backend, request_has_body) {
            active.push(mirror);
        }
    }
    active
}

pub(crate) async fn wait_for_request_mirrors(mirrors: &mut Vec<MirrorRequestSession>) {
    for mut mirror in mirrors.drain(..) {
        let Some(mut completion) = mirror.completion.take() else {
            continue;
        };

        match tokio::time::timeout(REQUEST_MIRROR_WAIT_TIMEOUT, &mut completion).await {
            Ok(Ok(())) => {}
            Ok(Err(err)) => {
                let failures = REQUEST_MIRROR_WAIT_JOIN_ERRORS.fetch_add(1, Ordering::Relaxed) + 1;
                if failures == 1 || failures.is_multiple_of(REQUEST_MIRROR_WAIT_WARN_INTERVAL) {
                    warn!(
                        backend = %mirror.backend_name,
                        failures,
                        error = %err,
                        "request mirror completion task exited unexpectedly"
                    );
                }
            }
            Err(_) => {
                let timeouts = REQUEST_MIRROR_WAIT_TIMEOUTS.fetch_add(1, Ordering::Relaxed) + 1;
                if timeouts == 1 || timeouts.is_multiple_of(REQUEST_MIRROR_WAIT_WARN_INTERVAL) {
                    warn!(
                        backend = %mirror.backend_name,
                        timeout_ms = REQUEST_MIRROR_WAIT_TIMEOUT.as_millis(),
                        timeouts,
                        "request mirror did not complete before the response finished"
                    );
                }
            }
        }
    }
}

pub(crate) async fn forward_body_chunk(
    mirror: &MirrorRequestSession,
    body: &Option<Bytes>,
    end_of_stream: bool,
) -> bool {
    if !mirror.forwards_body {
        return true;
    }

    if let Some(bytes) = body.as_ref().filter(|bytes| !bytes.is_empty()) {
        if !try_send_mirror_task(
            mirror,
            HttpTask::Body(Some(bytes.clone()), false),
            "body chunk",
        ) {
            return false;
        }
    }

    if end_of_stream && !try_send_mirror_task(mirror, HttpTask::Done, "end-of-stream marker") {
        return false;
    }

    true
}

fn mirror_subrequest(session: &Session) -> Option<&MirrorSubrequest> {
    session
        .subrequest_ctx
        .as_ref()?
        .user_ctx()?
        .as_ref()
        .downcast_ref::<MirrorSubrequest>()
}

async fn drain_mirror_response(
    mut rx: mpsc::Receiver<HttpTask>,
    mut proxy_error_rx: oneshot::Receiver<Box<pingora::Error>>,
    _permit: OwnedSemaphorePermit,
) {
    let mut proxy_error_pending = true;
    loop {
        tokio::select! {
            task = rx.recv() => {
                match task {
                    Some(HttpTask::Failed(err)) => {
                        let failures = REQUEST_MIRROR_FAILED_TASKS.fetch_add(1, Ordering::Relaxed) + 1;
                        if failures == 1
                            || failures.is_multiple_of(REQUEST_MIRROR_FAILURE_WARN_INTERVAL)
                        {
                            warn!(
                                error = %err,
                                failures,
                                "request mirror subrequest returned failed task"
                            );
                        }
                    }
                    Some(_) => {}
                    None => break,
                }
            }
            result = &mut proxy_error_rx, if proxy_error_pending => {
                proxy_error_pending = false;
                if let Ok(err) = result {
                    let failures = REQUEST_MIRROR_PROXY_ERRORS.fetch_add(1, Ordering::Relaxed) + 1;
                    if failures == 1
                        || failures.is_multiple_of(REQUEST_MIRROR_FAILURE_WARN_INTERVAL)
                    {
                        warn!(
                            error = %err,
                            failures,
                            "request mirror subrequest proxy error"
                        );
                    }
                }
            }
        }
    }
}

fn try_send_mirror_task(mirror: &MirrorRequestSession, task: HttpTask, task_name: &str) -> bool {
    match mirror.tx.try_send(task) {
        Ok(()) => true,
        Err(mpsc::error::TrySendError::Full(_)) => {
            let dropped = REQUEST_MIRROR_DROPPED_TASKS.fetch_add(1, Ordering::Relaxed) + 1;
            if dropped == 1 || dropped.is_multiple_of(REQUEST_MIRROR_DROP_WARN_INTERVAL) {
                warn!(
                    backend = %mirror.backend_name,
                    task = task_name,
                    dropped,
                    "dropping request mirror because the body forwarding queue is full"
                );
            }
            false
        }
        Err(mpsc::error::TrySendError::Closed(_)) => false,
    }
}

fn request_mirror_semaphore() -> &'static Arc<Semaphore> {
    REQUEST_MIRROR_SEMAPHORE.get_or_init(|| Arc::new(Semaphore::new(request_mirror_budget_limit())))
}

fn request_mirror_budget_limit() -> usize {
    REQUEST_MIRROR_LIMIT
        .get()
        .copied()
        .unwrap_or(MAX_ACTIVE_REQUEST_MIRRORS)
}

#[cfg(test)]
mod tests;
