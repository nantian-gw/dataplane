async fn wait_for_listener(port: u16) {
    for _ in 0..50 {
        if TcpStream::connect(("127.0.0.1", port)).await.is_ok() {
            return;
        }
        sleep(Duration::from_millis(20)).await;
    }
    panic!("listener on port {port} did not become ready");
}

async fn wait_for_log_contents(path: &PathBuf) -> String {
    wait_for_log_contents_matching(path, |contents| !contents.trim().is_empty()).await
}

async fn wait_for_log_contents_matching(
    path: &PathBuf,
    predicate: impl Fn(&str) -> bool,
) -> String {
    let mut last_contents = String::new();
    for _ in 0..20 {
        if let Ok(contents) = fs::read_to_string(path) {
            if predicate(&contents) {
                return contents;
            }
            last_contents = contents;
        }
        sleep(Duration::from_millis(50)).await;
    }

    if !last_contents.is_empty() {
        return last_contents;
    }

    fs::read_to_string(path).expect("access log file")
}

#[tokio::test]
async fn wait_for_log_contents_matching_waits_for_predicate() {
    let log_path = temp_log_path("wait-for-log-contents-matching");
    fs::write(&log_path, "text/plain 200\n").expect("initial log write");
    let writer_path = log_path.clone();
    let writer = tokio::spawn(async move {
        sleep(Duration::from_millis(25)).await;
        fs::write(&writer_path, "text/plain 200\ntext/plain -\n").expect("matching log write");
    });

    let log_contents =
        wait_for_log_contents_matching(&log_path, |contents| contents.contains("text/plain -"))
            .await;

    writer.await.expect("log writer task");
    assert!(log_contents.contains("text/plain 200"));
    assert!(log_contents.contains("text/plain -"));
    let _ = fs::remove_file(log_path);
}

async fn wait_for_traffic_snapshot<F>(
    traffic: &SharedTrafficStats,
    mut predicate: F,
) -> TrafficSnapshot
where
    F: FnMut(&TrafficSnapshot) -> bool,
{
    let mut snapshot = traffic.snapshot();
    for _ in 0..20 {
        snapshot = traffic.snapshot();
        if predicate(&snapshot) {
            return snapshot;
        }
        sleep(Duration::from_millis(50)).await;
    }

    snapshot
}

fn grpc_data_frame(payload: &[u8]) -> Vec<u8> {
    let mut frame = Vec::with_capacity(5 + payload.len());
    frame.push(0);
    frame.extend_from_slice(&(payload.len() as u32).to_be_bytes());
    frame.extend_from_slice(payload);
    frame
}

fn temp_log_path(prefix: &str) -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time")
        .as_nanos();
    std::env::temp_dir().join(format!("ntgw-http-{prefix}-{unique}.log"))
}

fn free_tcp_port() -> u16 {
    let listener = StdTcpListener::bind("127.0.0.1:0").expect("free port bind");
    listener.local_addr().expect("free port addr").port()
}

fn install_rustls_provider() {
    // Upstream upstream OpenSSL runtime does not require explicit Rustls provider setup.
}
