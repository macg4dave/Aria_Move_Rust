//! Streaming copy with durability guarantees.
//! - Writes to a newly created destination file (fails if it exists).
//! - Buffers I/O for fewer syscalls.
//! - Flushes and fsyncs to ensure data hits disk before returning.

use std::fs::{File, OpenOptions};
use std::io::{self, BufReader, BufWriter, Write};
use std::path::Path;

/// Copy `src` -> `dst` using buffered I/O, then fsync the destination.
/// Returns the number of bytes written.
/// Notes:
/// - `dst` is created with `create_new(true)` so we never clobber an existing file.
/// - Callers are responsible for syncing the parent directory after the final rename.
pub(super) fn copy_streaming(src: &Path, dst: &Path) -> io::Result<u64> {
    const BUF_SIZE: usize = 1024 * 1024; // 1 MiB buffers for fewer syscalls

    // Open source for reading (buffered)
    let src_f = File::open(src)?;
    let mut reader = BufReader::with_capacity(BUF_SIZE, src_f);

    // Create destination (fail if it already exists), buffered writer
    let dst_f = OpenOptions::new().write(true).create_new(true).open(dst)?;
    let mut writer = BufWriter::with_capacity(BUF_SIZE, dst_f);

    // Stream the copy
    let written = io::copy(&mut reader, &mut writer)?;

    // Ensure all buffered bytes are pushed to the OS, then fsync to disk
    writer.flush()?;
    writer.get_ref().sync_all()?;

    Ok(written)
}