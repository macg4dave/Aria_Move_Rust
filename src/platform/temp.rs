//! Shared temporary name helpers for platform modules.
//! Provides unique sibling filenames for atomic write operations.
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// Generate a unique hidden sibling temp name for config/log atomic writes.
/// Pattern: .aria_move.config.tmp.<pid>.<nanos>.<seq>
pub fn tmp_config_sibling_name(target: &Path) -> PathBuf {
    let pid = std::process::id();
    let nanos = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
    let seq = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let name = format!(".aria_move.config.tmp.{pid}.{nanos}.{seq}");
    target.parent().unwrap_or_else(|| Path::new(".")).join(name)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;
    use std::thread;

    #[test]
    fn uniqueness_concurrent() {
        let target = Path::new("dummy.xml");
        let mut handles = Vec::new();
        for _ in 0..32 { // modest concurrency
            let t = target.to_path_buf();
            handles.push(thread::spawn(move || tmp_config_sibling_name(&t)));
        }
        let mut set = HashSet::new();
        for h in handles { let p = h.join().unwrap(); assert!(set.insert(p)); }
        assert_eq!(set.len(), 32);
    }
}
