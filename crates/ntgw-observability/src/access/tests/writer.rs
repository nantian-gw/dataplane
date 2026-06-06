use std::borrow::Cow;

use std::{
    io::{self, Write},
    sync::{
        atomic::{AtomicU64, Ordering},
        mpsc::{Receiver, SyncSender},
        Arc,
    },
    time::{Duration, Instant},
};

#[test]
fn writes_access_logs_via_background_worker() {
    let path = temp_log_path("access-log");
    let options = AccessLogOptions {
        path: path.display().to_string(),
        ..AccessLogOptions::default()
    };
    let record = AccessLogRecord {
        event: "http_request".to_string(),
        route_name: Cow::Borrowed("orders"),
        ..AccessLogRecord::default()
    };

    write_access_log(&options, &BTreeMap::new(), &record).expect("log should be queued");
    flush_access_log(&path.display().to_string()).expect("log writer should flush");

    let contents = fs::read_to_string(&path).expect("log file");
    assert!(contents.contains("http_request"));
    assert!(contents.contains("\"routeName\":\"orders\""));

    let _ = fs::remove_file(path);
}

#[test]
fn drops_access_logs_when_queue_is_full_without_failing_callers() {
    let (tx, _rx) = mpsc::sync_channel(1);
    let writer = AccessLogWriter::new(tx);

    queue_access_log_line("stdout", &writer, "first".to_string())
        .expect("first log line should queue");
    queue_access_log_line("stdout", &writer, "second".to_string())
        .expect("full queue should not fail caller");

    let snapshot = writer.snapshot();
    assert_eq!(snapshot.queue_depth, 1);
    assert_eq!(snapshot.dropped_lines_total, 1);
}

#[test]
fn slow_access_log_sink_drops_without_blocking_callers() {
    let (write_started_tx, write_started_rx) = mpsc::sync_channel(1);
    let (release_write_tx, release_write_rx) = mpsc::sync_channel(1);
    let write_calls = Arc::new(AtomicU64::new(0));
    let writer = spawn_access_log_writer_for_test(
        "slow-sink",
        Box::new(BlockingWriter {
            first_write_started: write_started_tx,
            release_first_write: release_write_rx,
            write_calls: Arc::clone(&write_calls),
        }),
        1,
    )
    .expect("slow sink writer");

    queue_access_log_line("slow-sink", &writer, "first".to_string())
        .expect("first line should reach the slow sink");
    write_started_rx
        .recv_timeout(Duration::from_secs(1))
        .expect("slow sink should start writing the first line");

    let started = Instant::now();
    queue_access_log_line("slow-sink", &writer, "second".to_string())
        .expect("second line should queue behind slow sink");
    queue_access_log_line("slow-sink", &writer, "third".to_string())
        .expect("full queue should drop without failing caller");
    queue_access_log_line("slow-sink", &writer, "fourth".to_string())
        .expect("full queue should keep dropping without failing caller");
    assert!(
        started.elapsed() < Duration::from_millis(50),
        "slow sink backpressure should not block access log callers"
    );

    let snapshot = writer.snapshot();
    assert_eq!(snapshot.queue_depth, 2);
    assert_eq!(snapshot.dropped_lines_total, 2);

    release_write_tx
        .send(())
        .expect("release blocked slow sink write");
}

#[test]
fn access_log_worker_flushes_buffer_on_flush_command_not_each_line() {
    let flush_calls = Arc::new(AtomicU64::new(0));
    let writer = spawn_access_log_writer_for_test(
        "batched-sink",
        Box::new(FlushCountingWriter {
            flush_calls: Arc::clone(&flush_calls),
        }),
        8,
    )
    .expect("batched sink writer");

    queue_access_log_line("batched-sink", &writer, "first".to_string())
        .expect("line should queue");
    wait_for_queue_depth(&writer, 0);

    assert_eq!(
        flush_calls.load(Ordering::Relaxed),
        0,
        "line writes should not flush the sink until an explicit or batched flush"
    );

    let (done_tx, done_rx) = mpsc::sync_channel(1);
    writer
        .tx
        .send(AccessLogCommand::Flush(done_tx))
        .expect("flush command should queue");
    done_rx
        .recv_timeout(Duration::from_secs(1))
        .expect("flush command should complete");

    assert_eq!(flush_calls.load(Ordering::Relaxed), 1);
}

fn wait_for_queue_depth(writer: &AccessLogWriter, expected: u64) {
    for _ in 0..20 {
        if writer.snapshot().queue_depth == expected {
            return;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    assert_eq!(writer.snapshot().queue_depth, expected);
}

struct BlockingWriter {
    first_write_started: SyncSender<()>,
    release_first_write: Receiver<()>,
    write_calls: Arc<AtomicU64>,
}

impl Write for BlockingWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        if self.write_calls.fetch_add(1, Ordering::Relaxed) == 0 {
            let _ = self.first_write_started.send(());
            let _ = self.release_first_write.recv();
        }
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

struct FlushCountingWriter {
    flush_calls: Arc<AtomicU64>,
}

impl Write for FlushCountingWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        self.flush_calls.fetch_add(1, Ordering::Relaxed);
        Ok(())
    }
}
