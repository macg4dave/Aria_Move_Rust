use std::ffi::{OsStr, OsString};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

pub(super) fn claim_source(src: &Path) -> io::Result<PathBuf> {
    let pid = std::process::id();
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);

    let parent = src.parent().unwrap_or_else(|| Path::new(""));
    let fname = src.file_name().unwrap_or_else(|| OsStr::new("file"));

    let mut new_name: OsString = fname.to_os_string();
    new_name.push(format!(".aria_move.moving.{}.{}", pid, nanos));

    let claimed = parent.join(new_name);
    fs::rename(src, &claimed)?;
    Ok(claimed)
}