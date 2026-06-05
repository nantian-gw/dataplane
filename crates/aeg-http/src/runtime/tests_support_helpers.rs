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
    for _ in 0..20 {
        if let Ok(contents) = fs::read_to_string(path) {
            if !contents.trim().is_empty() {
                return contents;
            }
        }
        sleep(Duration::from_millis(50)).await;
    }

    fs::read_to_string(path).expect("access log file")
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
    std::env::temp_dir().join(format!("aeg-http-{prefix}-{unique}.log"))
}

fn free_tcp_port() -> u16 {
    let listener = StdTcpListener::bind("127.0.0.1:0").expect("free port bind");
    listener.local_addr().expect("free port addr").port()
}

fn install_rustls_provider() {
    // Upstream upstream OpenSSL runtime does not require explicit Rustls provider setup.
}
