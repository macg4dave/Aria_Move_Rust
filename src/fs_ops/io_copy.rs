//! Streaming copy with durability / configurability.
//!
//! Features:
//! - Writes to a newly created destination file (O_EXCL semantics; never clobbers).
//! - Buffered I/O with large (1 MiB) buffers to reduce syscall count.
//! - Optional write-through / full fsync for strong durability guarantees.
//! - Returns a `CopyResult` struct for richer instrumentation.
//!
//! Snapshot semantics: the source file is read once from start to EOF; if it grows
//! concurrently, the additional bytes are not included. Shrinks/truncation during
//! copy will surface as read errors or early EOF; caller can compare `bytes` to the
//! original metadata length if stricter validation is required.

use std::fs::{File, OpenOptions};
use std::io::{self, BufReader, BufWriter, Read, Write};
use std::path::Path;

/// Durability mode controlling post-write flush behavior.
#[derive(Clone, Copy, Debug)]
pub enum DurabilityMode {
    /// Ensure written data reaches the OS page cache (`flush`), but do not force
    /// a disk barrier. Fastest; may lose data on sudden power loss.
    Data,
    /// Force data and metadata to stable storage (`sync_all`). Highest integrity.
    Full,
}

/// Result of a streaming copy operation.
#[derive(Debug, Clone, Copy)]
pub struct CopyResult {
    /// Total bytes copied from source to destination.
    pub bytes: u64,
    /// Size of the buffer used for copying (for perf metrics).
    pub buf_size: usize,
    /// Durability mode applied.
    pub mode: DurabilityMode,
}

/// Copy `src` -> `dst` using buffered I/O, then fsync the destination.
/// Returns the number of bytes written.
/// Notes:
/// - `dst` is created with `create_new(true)` so we never clobber an existing file.
/// - Callers are responsible for syncing the parent directory after the final rename.
pub(super) fn copy_streaming(src: &Path, dst: &Path) -> io::Result<u64> {
    // Backwards compatibility shim returning just bytes with Full semantics.
    let res = copy_streaming_ex(src, dst, DurabilityMode::Full)?;
    Ok(res.bytes)
}

/// Extended streaming copy with selectable durability.
pub(super) fn copy_streaming_ex(
    src: &Path,
    dst: &Path,
    mode: DurabilityMode,
) -> io::Result<CopyResult> {
    const BUF_SIZE: usize = 1024 * 1024; // 1 MiB buffers

    // Fast-path: on macOS, try APFS clonefile to CoW-clone the file.
    // This creates the destination path atomically and is O(1) for metadata.
    #[cfg(target_os = "macos")]
    {
        use std::ffi::CString;
        use std::os::unix::ffi::OsStrExt;
        unsafe {
            let src_c = CString::new(src.as_os_str().as_bytes()).unwrap();
            let dst_c = CString::new(dst.as_os_str().as_bytes()).unwrap();
            // clonefile returns 0 on success, -1 on error with errno set.
            let rc = libc::clonefile(src_c.as_ptr(), dst_c.as_ptr(), 0);
            if rc == 0 {
                let bytes = File::open(src)?.metadata()?.len();
                // Apply durability if requested
                if matches!(mode, DurabilityMode::Full) {
                    let f = File::options().read(true).write(false).open(dst)?;
                    f.sync_all()?;
                }
                return Ok(CopyResult { bytes, buf_size: BUF_SIZE, mode });
            } else {
                // On errors like EXDEV/ENOTSUP/EPERM fall through to streaming; EEXIST should be
                // impossible here since we always choose a unique temp name in higher layers.
            }
        }
    }

    // Open source file for streaming or Linux fast-path.
    let mut src_f = File::open(src)?;

    // Destination options
    let mut opts = OpenOptions::new();
    opts.write(true).create_new(true);

    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        // FILE_FLAG_WRITE_THROUGH = 0x80000000 improves durability for Full mode.
        if matches!(mode, DurabilityMode::Full) {
            const FILE_FLAG_WRITE_THROUGH: u32 = 0x8000_0000;
            opts.custom_flags(FILE_FLAG_WRITE_THROUGH);
        }
    }

    let mut dst_f = opts.open(dst)?;

    // Fast-path: on Linux, try copy_file_range for in-kernel copy when supported.
    #[cfg(target_os = "linux")]
    {
        use std::os::unix::io::AsRawFd;
        // Try once with a large chunk size to detect support; if unsupported and no bytes copied,
        // we'll fall back to streaming.
        let mut total: u64 = 0;
        let chunk: usize = 16 * 1024 * 1024; // 16 MiB per call
        loop {
            let rc = unsafe {
                libc::copy_file_range(
                    src_f.as_raw_fd(),
                    std::ptr::null_mut(),
                    dst_f.as_raw_fd(),
                    std::ptr::null_mut(),
                    chunk,
                    0,
                )
            };
            if rc > 0 {
                total += rc as u64;
                continue;
            } else if rc == 0 {
                // EOF reached
                if matches!(mode, DurabilityMode::Full) {
                    dst_f.sync_all()?;
                }
                return Ok(CopyResult { bytes: total, buf_size: BUF_SIZE, mode });
            } else {
                // Error; if no bytes copied and error indicates unsupported, fall back.
                let err = io::Error::last_os_error();
                if total == 0 {
                    if let Some(code) = err.raw_os_error() {
                        if code == libc::EXDEV
                            || code == libc::ENOSYS
                            || code == libc::EINVAL
                            || code == libc::EPERM
                        {
                            // Unsupported; break to streaming fallback
                        } else {
                            return Err(err);
                        }
                    } else {
                        return Err(err);
                    }
                } else {
                    // Partial copy then error: return error; higher level will cleanup temp.
                    return Err(err);
                }
                break; // fallback
            }
        }
    }

    // Streaming fallback (or non-Linux/non-macOS default): buffered io::copy
    let mut reader = BufReader::with_capacity(BUF_SIZE, src_f);
    let mut writer = BufWriter::with_capacity(BUF_SIZE, dst_f);
    let bytes = io::copy(&mut reader, &mut writer)?;
    writer.flush()?;

    if matches!(mode, DurabilityMode::Full) {
        writer.get_ref().sync_all()?;
    }

    Ok(CopyResult { bytes, buf_size: BUF_SIZE, mode })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::io::Write as _;
    use tempfile::tempdir;

    #[test]
    fn copy_small_file_ok() {
        let dir = tempdir().unwrap();
        let src_path = dir.path().join("src.txt");
        let dst_path = dir.path().join("dst.txt");

        // Write some content
        let data = b"hello world";
        fs::write(&src_path, data).unwrap();

        let n = copy_streaming(&src_path, &dst_path).unwrap();
        assert_eq!(n, data.len() as u64);

        let got = fs::read(&dst_path).unwrap();
        assert_eq!(&got, data);
    }

    #[test]
    fn copy_zero_length_ok() {
        let dir = tempdir().unwrap();
        let src_path = dir.path().join("empty");
        let dst_path = dir.path().join("out");
        File::create(&src_path).unwrap(); // empty file

        let n = copy_streaming(&src_path, &dst_path).unwrap();
        assert_eq!(n, 0);
        let meta = fs::metadata(&dst_path).unwrap();
        assert_eq!(meta.len(), 0);
    }

    #[test]
    fn fails_if_dest_exists() {
        let dir = tempdir().unwrap();
        let src_path = dir.path().join("src");
        let dst_path = dir.path().join("dst");
        fs::write(&src_path, b"data").unwrap();
        // Pre-create destination
        let mut f = File::create(&dst_path).unwrap();
        f.write_all(b"x").unwrap();
        drop(f);

        let err = copy_streaming(&src_path, &dst_path).unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::AlreadyExists);
    }

    #[test]
    fn large_file_copy_boundary() {
        let dir = tempdir().unwrap();
        let src = dir.path().join("big.bin");
        let dst = dir.path().join("big.out");

        // Size > 2 * BUF_SIZE + 123 to cross multiple boundaries
        const BUF_SIZE: usize = 1024 * 1024;
        let size = 2 * BUF_SIZE + 123;
        let mut data = vec![0u8; size];
        for (i, b) in data.iter_mut().enumerate() {
            *b = (i % 251) as u8; // pseudo pattern
        }
        fs::write(&src, &data).unwrap();

        let res = copy_streaming_ex(&src, &dst, DurabilityMode::Data).unwrap();
        assert_eq!(res.bytes as usize, size);
        assert_eq!(res.buf_size, BUF_SIZE);
        assert!(matches!(res.mode, DurabilityMode::Data));

        let out = fs::read(&dst).unwrap();
        assert_eq!(out, data);
    }

    #[test]
    fn durability_full_syncs() {
        let dir = tempdir().unwrap();
        let src = dir.path().join("d.txt");
        let dst = dir.path().join("d.out");
        fs::write(&src, b"abcdef").unwrap();
        let res = copy_streaming_ex(&src, &dst, DurabilityMode::Full).unwrap();
        assert_eq!(res.bytes, 6);
        assert!(matches!(res.mode, DurabilityMode::Full));
        let got = fs::read(&dst).unwrap();
        assert_eq!(got, b"abcdef");
    }
}