//! Platform memory availability, with Linux container limits.

/// Leave headroom for the UI and other programs, even on large machines.
pub fn all_rows_budget() -> u64 {
    (available_memory().unwrap_or(256 << 20) / 4).min(2 << 30)
}

#[cfg(any(target_os = "windows", target_os = "macos"))]
fn available_memory() -> Option<u64> {
    let mut system = sysinfo::System::new();
    system.refresh_memory_specifics(sysinfo::MemoryRefreshKind::nothing().with_ram());
    (system.total_memory() > 0).then(|| system.available_memory())
}

#[cfg(not(any(target_os = "linux", target_os = "windows", target_os = "macos")))]
fn available_memory() -> Option<u64> {
    None
}

#[cfg(target_os = "linux")]
fn available_memory() -> Option<u64> {
    let available = std::fs::read_to_string("/proc/meminfo")
        .ok()
        .and_then(|contents| {
            contents.lines().find_map(|line| {
                line.strip_prefix("MemAvailable:")
                    .and_then(|value| value.split_whitespace().next()?.parse::<u64>().ok())
                    .map(|kib| kib.saturating_mul(1024))
            })
        });
    let mut available = available.unwrap_or(256 << 20);
    // Respect cgroup v2 memory limits as well as the host's free memory.
    let cgroup = std::fs::read_to_string("/proc/self/cgroup")
        .ok()
        .and_then(|text| {
            text.lines()
                .find_map(|line| line.strip_prefix("0::").map(str::to_owned))
        });
    let mut directory = std::path::PathBuf::from("/sys/fs/cgroup");
    if let Some(group) = cgroup {
        directory.push(group.trim_start_matches('/'));
    }
    loop {
        let read = |name| {
            std::fs::read_to_string(directory.join(name))
                .ok()?
                .trim()
                .parse::<u64>()
                .ok()
        };
        if let (Some(limit), Some(used)) = (read("memory.max"), read("memory.current")) {
            available = available.min(limit.saturating_sub(used));
        }
        if directory == std::path::Path::new("/sys/fs/cgroup") || !directory.pop() {
            break;
        }
    }
    Some(available)
}

#[cfg(test)]
mod tests {
    #[test]
    fn memory_query_is_bounded_and_available_on_supported_hosts() {
        let available = super::available_memory().expect("native memory query must be available");
        assert!(super::all_rows_budget() <= 2 << 30);
        // Availability can change between calls; do not compare live snapshots.
        assert!(available > 0);
    }
}
