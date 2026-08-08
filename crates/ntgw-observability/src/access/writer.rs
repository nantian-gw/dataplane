use std::{
    collections::HashMap,
    fs::OpenOptions,
    io::{self, BufWriter, Write},
    sync::{
        Arc, OnceLock,
        atomic::{AtomicU64, Ordering},
        mpsc::{self, Receiver, RecvTimeoutError, SyncSender, TrySendError},
    },
    thread,
    time::{Duration, Instant},
};

use anyhow::{Result, anyhow};
use parking_lot::Mutex;
use tracing::error;
use tracing::warn;

use super::{AccessLogWriterSnapshot, FlushSyncSender};

static ACCESS_LOG_WRITERS: OnceLock<Mutex<HashMap<String, AccessLogWriter>>> = OnceLock::new();
const ACCESS_LOG_QUEUE_CAPACITY: usize = 8_192;
const ACCESS_LOG_DROP_WARN_INTERVAL: u64 = 1_024;
const ACCESS_LOG_BUFFER_CAPACITY: usize = 64 * 1024;
const ACCESS_LOG_FLUSH_BATCH_LINES: u64 = 1_024;
const ACCESS_LOG_FLUSH_INTERVAL_MS: u64 = 100;

#[derive(Clone)]
pub(super) struct AccessLogWriter {
    pub(super) tx: SyncSender<AccessLogCommand>,
    pub(super) stats: Arc<AccessLogWriterStats>,
}

#[derive(Default)]
pub(super) struct AccessLogWriterStats {
    queue_depth: AtomicU64,
    dropped_lines_total: AtomicU64,
    flushes_total: AtomicU64,
    flush_latency_ms_total: AtomicU64,
    flush_latency_ms_max: AtomicU64,
    sink_errors_total: AtomicU64,
}

impl AccessLogWriter {
    pub(super) fn new(tx: SyncSender<AccessLogCommand>) -> Self {
        Self {
            tx,
            stats: Arc::new(AccessLogWriterStats::default()),
        }
    }

    #[cfg(test)]
    pub(super) fn snapshot(&self) -> AccessLogWriterSnapshot {
        let mut snapshot = self.stats.snapshot();
        snapshot.writers = 1;
        snapshot
    }
}

impl AccessLogWriterStats {
    fn snapshot(&self) -> AccessLogWriterSnapshot {
        AccessLogWriterSnapshot {
            writers: 0,
            queue_depth: self.queue_depth.load(Ordering::Relaxed),
            dropped_lines_total: self.dropped_lines_total.load(Ordering::Relaxed),
            flushes_total: self.flushes_total.load(Ordering::Relaxed),
            flush_latency_ms_total: self.flush_latency_ms_total.load(Ordering::Relaxed),
            flush_latency_ms_max: self.flush_latency_ms_max.load(Ordering::Relaxed),
            sink_errors_total: self.sink_errors_total.load(Ordering::Relaxed),
        }
    }

    fn increment_queue_depth(&self) {
        self.queue_depth.fetch_add(1, Ordering::Relaxed);
    }

    fn decrement_queue_depth(&self) {
        let _ = self
            .queue_depth
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |value| {
                Some(value.saturating_sub(1))
            });
    }

    fn record_drop(&self) -> u64 {
        self.dropped_lines_total.fetch_add(1, Ordering::Relaxed) + 1
    }

    fn record_flush_latency(&self, latency_ms: u64) {
        self.flushes_total.fetch_add(1, Ordering::Relaxed);
        self.flush_latency_ms_total
            .fetch_add(latency_ms, Ordering::Relaxed);
        self.flush_latency_ms_max
            .fetch_max(latency_ms, Ordering::Relaxed);
    }

    fn record_sink_error(&self) {
        self.sink_errors_total.fetch_add(1, Ordering::Relaxed);
    }
}

pub(super) enum AccessLogCommand {
    Line(String),
    Flush(FlushSyncSender),
}

pub(super) fn emit_access_log_line(path: &str, line: String) -> Result<()> {
    let writer = access_log_writer(path)?;
    queue_access_log_line(path, &writer, line)
}

pub(super) fn queue_access_log_line(
    path: &str,
    writer: &AccessLogWriter,
    line: String,
) -> Result<()> {
    writer.stats.increment_queue_depth();
    match writer.tx.try_send(AccessLogCommand::Line(line)) {
        Ok(()) => Ok(()),
        Err(TrySendError::Full(_)) => {
            writer.stats.decrement_queue_depth();
            let dropped = writer.stats.record_drop();
            if dropped == 1 || dropped.is_multiple_of(ACCESS_LOG_DROP_WARN_INTERVAL) {
                warn!(
                    path = %path,
                    dropped = dropped,
                    capacity = ACCESS_LOG_QUEUE_CAPACITY,
                    "dropping access log lines because writer queue is full"
                );
            }
            Ok(())
        }
        Err(TrySendError::Disconnected(err)) => {
            writer.stats.decrement_queue_depth();
            reset_access_log_writer(path);
            let writer = access_log_writer(path)?;
            let line = match err {
                AccessLogCommand::Line(line) => line,
                AccessLogCommand::Flush(_) => {
                    unreachable!("line send cannot disconnect with flush command")
                }
            };
            writer.stats.increment_queue_depth();
            if writer.tx.send(AccessLogCommand::Line(line)).is_err() {
                writer.stats.decrement_queue_depth();
                return Err(anyhow!("access log writer for {path} is unavailable"));
            }
            Ok(())
        }
    }
}

pub(super) fn reset_access_log_writer(path: &str) {
    if let Some(registry) = ACCESS_LOG_WRITERS.get() {
        registry.lock().remove(path);
    }
}

pub(super) fn flush_access_log(path: &str) -> Result<()> {
    let writer = access_log_writer(path)?;
    let (done_tx, done_rx) = mpsc::sync_channel(1);
    writer
        .tx
        .send(AccessLogCommand::Flush(done_tx))
        .map_err(|_| anyhow!("access log writer for {path} is unavailable"))?;
    done_rx
        .recv()
        .map_err(|_| anyhow!("access log writer for {path} did not flush"))
}

pub(super) fn access_log_writer_snapshot() -> AccessLogWriterSnapshot {
    let mut snapshot = AccessLogWriterSnapshot::default();
    if let Some(registry) = ACCESS_LOG_WRITERS.get() {
        let state = registry.lock();
        snapshot.writers = state.len() as u64;
        for writer in state.values() {
            let writer_snapshot = writer.stats.snapshot();
            snapshot.queue_depth += writer_snapshot.queue_depth;
            snapshot.dropped_lines_total += writer_snapshot.dropped_lines_total;
            snapshot.flushes_total += writer_snapshot.flushes_total;
            snapshot.flush_latency_ms_total += writer_snapshot.flush_latency_ms_total;
            snapshot.flush_latency_ms_max = snapshot
                .flush_latency_ms_max
                .max(writer_snapshot.flush_latency_ms_max);
            snapshot.sink_errors_total += writer_snapshot.sink_errors_total;
        }
    }
    snapshot
}

fn access_log_writer(path: &str) -> Result<AccessLogWriter> {
    let registry = ACCESS_LOG_WRITERS.get_or_init(|| Mutex::new(HashMap::new()));
    if let Some(writer) = registry.lock().get(path).cloned() {
        return Ok(writer);
    }

    let writer = spawn_access_log_writer(path)?;
    let mut state = registry.lock();
    if let Some(existing) = state.get(path).cloned() {
        return Ok(existing);
    }
    state.insert(path.to_string(), writer.clone());
    Ok(writer)
}

fn spawn_access_log_writer(path: &str) -> Result<AccessLogWriter> {
    let path = path.to_string();
    let worker_path = path.clone();
    let (tx, rx) = mpsc::sync_channel(ACCESS_LOG_QUEUE_CAPACITY);
    let (ready_tx, ready_rx) = mpsc::sync_channel(1);
    let access_log_writer = AccessLogWriter::new(tx);
    let worker_stats = Arc::clone(&access_log_writer.stats);
    thread::Builder::new()
        .name(format!("ntgw-access-log-{}", sanitize_worker_name(&path)))
        .spawn(move || {
            let writer = match create_access_log_target(&worker_path) {
                Ok(writer) => {
                    let _ = ready_tx.send(Ok(()));
                    writer
                }
                Err(err) => {
                    let _ = ready_tx.send(Err(err.to_string()));
                    return;
                }
            };

            run_access_log_worker(worker_path, writer, rx, worker_stats);
        })?;

    match ready_rx
        .recv()
        .map_err(|_| anyhow!("access log writer for {path} failed to initialize"))?
    {
        Ok(()) => Ok(access_log_writer),
        Err(err) => Err(anyhow!(err)),
    }
}

pub(super) struct NullSink;

impl std::io::Write for NullSink {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

pub(super) enum LogTarget {
    File(std::fs::File),
    Stdout(std::io::Stdout),
    Stderr(std::io::Stderr),
    #[expect(dead_code, reason = "reserved for future use")]
    Null(NullSink),
    #[cfg(test)]
    Custom(Box<dyn Write + Send>),
}

impl std::io::Write for LogTarget {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        match self {
            LogTarget::File(f) => f.write(buf),
            LogTarget::Stdout(s) => s.write(buf),
            LogTarget::Stderr(s) => s.write(buf),
            LogTarget::Null(_) => Ok(buf.len()),
            #[cfg(test)]
            LogTarget::Custom(w) => w.write(buf),
        }
    }

    fn flush(&mut self) -> std::io::Result<()> {
        match self {
            LogTarget::File(f) => f.flush(),
            LogTarget::Stdout(s) => s.flush(),
            LogTarget::Stderr(s) => s.flush(),
            LogTarget::Null(_) => Ok(()),
            #[cfg(test)]
            LogTarget::Custom(w) => w.flush(),
        }
    }
}

#[cfg(test)]
pub(super) fn spawn_access_log_writer_for_test(
    path: &str,
    writer: LogTarget,
    capacity: usize,
) -> Result<AccessLogWriter> {
    let (tx, rx) = mpsc::sync_channel(capacity);
    let access_log_writer = AccessLogWriter::new(tx);
    let worker_stats = Arc::clone(&access_log_writer.stats);
    let worker_path = path.to_string();
    thread::Builder::new()
        .name(format!(
            "ntgw-access-log-test-{}",
            sanitize_worker_name(path)
        ))
        .spawn(move || {
            run_access_log_worker(
                worker_path,
                BufWriter::with_capacity(1, writer),
                rx,
                worker_stats,
            );
        })?;
    Ok(access_log_writer)
}

fn run_access_log_worker(
    worker_path: String,
    mut writer: BufWriter<LogTarget>,
    rx: Receiver<AccessLogCommand>,
    worker_stats: Arc<AccessLogWriterStats>,
) {
    let mut pending_lines = 0_u64;

    loop {
        let command = if pending_lines == 0 {
            match rx.recv() {
                Ok(command) => command,
                Err(_) => break,
            }
        } else {
            match rx.recv_timeout(Duration::from_millis(ACCESS_LOG_FLUSH_INTERVAL_MS)) {
                Ok(command) => command,
                Err(RecvTimeoutError::Timeout) => {
                    flush_access_log_writer(&mut writer, &worker_stats);
                    pending_lines = 0;
                    continue;
                }
                Err(RecvTimeoutError::Disconnected) => break,
            }
        };

        match command {
            AccessLogCommand::Line(line) => {
                let wrote_line = match write_access_log_line(&mut writer, &line) {
                    Ok(()) => true,
                    Err(err) => {
                        worker_stats.record_sink_error();
                        if recover_access_log_target(&worker_path, &mut writer, &line).is_err() {
                            error!(%worker_path, %err, "failed to emit access log");
                            false
                        } else {
                            true
                        }
                    }
                };
                if wrote_line {
                    pending_lines += 1;
                }
                if pending_lines >= ACCESS_LOG_FLUSH_BATCH_LINES {
                    flush_access_log_writer(&mut writer, &worker_stats);
                    pending_lines = 0;
                }
                worker_stats.decrement_queue_depth();
            }
            AccessLogCommand::Flush(done) => {
                flush_access_log_writer(&mut writer, &worker_stats);
                pending_lines = 0;
                let _ = done.send(());
            }
        }
    }

    if pending_lines > 0 {
        flush_access_log_writer(&mut writer, &worker_stats);
    }
}

fn flush_access_log_writer(writer: &mut BufWriter<LogTarget>, worker_stats: &AccessLogWriterStats) {
    let started = Instant::now();
    if writer.flush().is_err() {
        worker_stats.record_sink_error();
    }
    let latency_ms = started.elapsed().as_millis().min(u64::MAX as u128) as u64;
    worker_stats.record_flush_latency(latency_ms);
}

fn sanitize_worker_name(path: &str) -> String {
    let sanitized = path
        .chars()
        .map(|ch| match ch {
            '/' | '\\' | ':' | '.' | ' ' => '_',
            _ => ch,
        })
        .collect::<String>();
    if sanitized.is_empty() {
        "default".to_string()
    } else {
        sanitized
    }
}

fn create_access_log_target(path: &str) -> Result<BufWriter<LogTarget>> {
    let writer = match path {
        "stdout" => LogTarget::Stdout(io::stdout()),
        "stderr" => LogTarget::Stderr(io::stderr()),
        _ => LogTarget::File(OpenOptions::new().create(true).append(true).open(path)?),
    };
    Ok(BufWriter::with_capacity(ACCESS_LOG_BUFFER_CAPACITY, writer))
}

fn write_access_log_line(writer: &mut BufWriter<LogTarget>, line: &str) -> io::Result<()> {
    writer.write_all(line.as_bytes())?;
    writer.write_all(b"\n")
}

fn recover_access_log_target(
    path: &str,
    writer: &mut BufWriter<LogTarget>,
    line: &str,
) -> Result<()> {
    if matches!(path, "stdout" | "stderr") {
        return write_access_log_line(writer, line).map_err(Into::into);
    }

    *writer = create_access_log_target(path)?;
    write_access_log_line(writer, line)?;
    Ok(())
}
