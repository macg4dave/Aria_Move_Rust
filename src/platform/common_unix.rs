//! Common Unix helpers shared by macOS and other Unix targets.
//! Includes atomic write with 0600 mode and parent directory fsync.

use anyhow::{Context, Result};
use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::os::unix::fs::OpenOptionsExt;
use std::path::Path;

use super::temp::tmp_config_sibling_name;

/// Atomically write `contents` to `path` with 0600 permissions on the file,
/// ensuring durability by fsync-ing the temp file and the parent directory.
///
/// Steps:
/// - Ensure parent directory exists
/// - Create unique hidden temp sibling with mode 0600 and O_EXCL semantics
/// - Write contents, fsync temp, rename to destination, fsync parent dir
/// - On failure, remove the temp file best-effort and return the error
pub fn atomic_write_0600(path: &Path, contents: &[u8]) -> Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "config path has no parent"))?;
    fs::create_dir_all(parent).with_context(|| format!("create parent '{}'", parent.display()))?;

    let tmp = tmp_config_sibling_name(path);

    let mut f = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&tmp)
        .with_context(|| format!("create temp '{}'", tmp.display()))?;
    f.write_all(contents).context("write temp")?;
    f.sync_all().context("fsync temp")?;
    drop(f);

    if let Err(e) = fs::rename(&tmp, path) {
        let _ = fs::remove_file(&tmp);
        return Err(e)
            .with_context(|| format!("rename '{}' -> '{}'", tmp.display(), path.display()));
    }

    let dir_file =
        File::open(parent).with_context(|| format!("open dir '{}'", parent.display()))?;
    dir_file.sync_all().context("fsync parent dir")?;
    Ok(())
}
