#[cfg(target_os = "linux")]
use std::{fs, path::Path, sync::OnceLock};

#[derive(Debug, Clone, Default, PartialEq)]
pub struct ProcessSnapshot {
    pub cpu_seconds_total: Option<f64>,
    pub resident_memory_bytes: Option<u64>,
    pub virtual_memory_bytes: Option<u64>,
    pub open_fds: Option<u64>,
    pub max_fds: Option<u64>,
    pub threads: Option<u64>,
}

pub fn snapshot_process() -> ProcessSnapshot {
    #[cfg(target_os = "linux")]
    {
        snapshot_linux_process(Path::new("/proc/self"))
    }

    #[cfg(not(target_os = "linux"))]
    {
        ProcessSnapshot::default()
    }
}

#[cfg(target_os = "linux")]
fn snapshot_linux_process(proc_self: &Path) -> ProcessSnapshot {
    let mut snapshot = ProcessSnapshot {
        open_fds: fs::read_dir(proc_self.join("fd"))
            .ok()
            .map(|entries| entries.count() as u64),
        ..ProcessSnapshot::default()
    };

    if let Ok(status) = fs::read_to_string(proc_self.join("status")) {
        apply_status(&mut snapshot, &status);
    }
    if let Ok(stat) = fs::read_to_string(proc_self.join("stat"))
        && let Some((user_ticks, system_ticks)) = parse_proc_stat_cpu_ticks(&stat)
    {
        snapshot.cpu_seconds_total =
            Some((user_ticks.saturating_add(system_ticks)) as f64 / clock_ticks_per_second());
    }
    if let Ok(limits) = fs::read_to_string(proc_self.join("limits")) {
        snapshot.max_fds = parse_max_open_files_limit(&limits);
    }

    snapshot
}

#[cfg(target_os = "linux")]
fn apply_status(snapshot: &mut ProcessSnapshot, status: &str) {
    for line in status.lines() {
        if let Some(value) = parse_status_value(line, "VmRSS:") {
            snapshot.resident_memory_bytes = Some(value.saturating_mul(1024));
        }
        if let Some(value) = parse_status_value(line, "VmSize:") {
            snapshot.virtual_memory_bytes = Some(value.saturating_mul(1024));
        }
        if let Some(value) = parse_status_value(line, "Threads:") {
            snapshot.threads = Some(value);
        }
    }
}

#[cfg(target_os = "linux")]
fn parse_status_value(line: &str, key: &str) -> Option<u64> {
    line.strip_prefix(key)
        .and_then(|value| value.split_whitespace().next())
        .and_then(|value| value.parse::<u64>().ok())
}

#[cfg(target_os = "linux")]
fn parse_proc_stat_cpu_ticks(stat: &str) -> Option<(u64, u64)> {
    let rest = stat.rsplit_once(") ")?.1;
    let fields = rest.split_whitespace().collect::<Vec<_>>();
    let user_ticks = fields.get(11)?.parse::<u64>().ok()?;
    let system_ticks = fields.get(12)?.parse::<u64>().ok()?;
    Some((user_ticks, system_ticks))
}

#[cfg(target_os = "linux")]
fn parse_max_open_files_limit(limits: &str) -> Option<u64> {
    limits.lines().find_map(|line| {
        let value = line.strip_prefix("Max open files")?;
        value
            .split_whitespace()
            .next()
            .and_then(|soft_limit| soft_limit.parse::<u64>().ok())
    })
}

#[cfg(target_os = "linux")]
fn clock_ticks_per_second() -> f64 {
    static CLOCK_TICKS_PER_SECOND: OnceLock<f64> = OnceLock::new();

    *CLOCK_TICKS_PER_SECOND.get_or_init(|| {
        use nix::unistd::{SysconfVar, sysconf};

        match sysconf(SysconfVar::CLK_TCK) {
            Ok(Some(value)) if value > 0 => value as f64,
            _ => 100.0,
        }
    })
}

#[cfg(all(test, target_os = "linux"))]
mod tests {
    use super::*;

    #[test]
    fn parses_linux_status_memory_and_thread_fields() {
        let mut snapshot = ProcessSnapshot::default();

        apply_status(
            &mut snapshot,
            "Name:\tntgw\nVmSize:\t  1234 kB\nVmRSS:\t  567 kB\nThreads:\t9\n",
        );

        assert_eq!(snapshot.virtual_memory_bytes, Some(1_263_616));
        assert_eq!(snapshot.resident_memory_bytes, Some(580_608));
        assert_eq!(snapshot.threads, Some(9));
    }

    #[test]
    fn parses_cpu_ticks_after_parenthesized_command_name() {
        let stat = "1234 (ntgw app) S 1 2 3 4 5 6 7 8 9 10 42 58 14 15";

        assert_eq!(parse_proc_stat_cpu_ticks(stat), Some((42, 58)));
    }

    #[test]
    fn parses_soft_open_fd_limit() {
        let limits = "Limit                     Soft Limit           Hard Limit           Units\nMax open files            1048576              1048576              files\n";

        assert_eq!(parse_max_open_files_limit(limits), Some(1_048_576));
    }

    #[test]
    fn snapshots_linux_proc_layout() {
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system time")
            .as_nanos();
        let root = std::env::temp_dir().join(format!("ntgw-process-test-{unique}"));
        let fd_dir = root.join("fd");
        fs::create_dir_all(&fd_dir).expect("fd dir");
        fs::write(fd_dir.join("0"), "").expect("fd sample");
        fs::write(
            root.join("status"),
            "Name:\tntgw\nVmSize:\t  100 kB\nVmRSS:\t  25 kB\nThreads:\t3\n",
        )
        .expect("status");
        fs::write(root.join("stat"), "99 (ntgw) S 0 0 0 0 0 0 0 0 0 0 25 75").expect("stat");
        fs::write(
            root.join("limits"),
            "Limit                     Soft Limit           Hard Limit           Units\nMax open files            4096                 4096                 files\n",
        )
        .expect("limits");

        let snapshot = snapshot_linux_process(&root);

        assert!(snapshot.cpu_seconds_total.unwrap_or_default() > 0.0);
        assert_eq!(snapshot.resident_memory_bytes, Some(25 * 1024));
        assert_eq!(snapshot.virtual_memory_bytes, Some(100 * 1024));
        assert_eq!(snapshot.open_fds, Some(1));
        assert_eq!(snapshot.max_fds, Some(4096));
        assert_eq!(snapshot.threads, Some(3));

        fs::remove_dir_all(root).expect("cleanup");
    }
}
