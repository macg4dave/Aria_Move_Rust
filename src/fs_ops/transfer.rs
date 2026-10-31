//! Byte transfer, verification and change detection for regular files.
use crate::{platform, shutdown};
use anyhow::{Context, Result, ensure};
use std::fs::{File, Metadata};
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::path::Path;
use std::time::SystemTime;

const BUFFER_SIZE: usize = 1024 * 1024;

/// Identity, size and modification time of a file or directory. Directories
/// record only identity, since their size and mtime change with their children.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Snapshot {
    pub(super) len: u64,
    modified: Option<SystemTime>,
    identity: (u64, u64),
}

impl Snapshot {
    pub(super) fn new(file: &File, meta: &Metadata) -> Result<Self> {
        let directory = meta.is_dir();
        Ok(Self {
            len: if directory { 0 } else { meta.len() },
            modified: if directory {
                None
            } else {
                Some(meta.modified()?)
            },
            identity: platform::file_identity(file, meta)?,
        })
    }

    pub(super) fn of(file: &File) -> Result<Self> {
        Self::new(file, &file.metadata()?)
    }

    /// Fail if the open file no longer matches this snapshot.
    pub(super) fn check(&self, file: &File, path: &Path) -> Result<()> {
        ensure!(
            *self == Self::of(file)?,
            "source changed during the move: {}",
            path.display()
        );
        Ok(())
    }
}

/// Copy `src` into `destination`, durably, and verify the bytes before
/// returning. Fails if the source changes during the copy.
pub(super) fn copy_file(src: &Path, destination: &mut File) -> Result<(Snapshot, Metadata)> {
    let mut source = platform::open_regular_read(src)
        .map_err(|e| crate::errors::io_error("open source", src, e))?;
    let meta = source.metadata()?;
    let snapshot = Snapshot::new(&source, &meta)?;
    let written = copy_stream(&mut source, destination)?;
    ensure!(
        written == snapshot.len,
        "source size changed during copy: {}",
        src.display()
    );
    destination.sync_all()?;
    verify_equal(&mut source, destination)
        .with_context(|| format!("verify copy of {}", src.display()))?;
    snapshot.check(&source, src)?;
    Ok((snapshot, meta))
}

/// Cancellable copy from the start of `source` to the current end of `destination`.
fn copy_stream(source: &mut File, destination: &mut File) -> Result<u64> {
    source.seek(SeekFrom::Start(0))?;
    let mut buffer = vec![0; BUFFER_SIZE];
    let mut total = 0u64;
    loop {
        shutdown::check()?;
        let count = match source.read(&mut buffer) {
            Ok(0) => return Ok(total),
            Ok(count) => count,
            Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
            Err(e) => return Err(e.into()),
        };
        destination.write_all(&buffer[..count])?;
        total += count as u64;
    }
}

/// Byte-for-byte comparison from the start of both files, including length.
fn verify_equal(source: &mut File, destination: &mut File) -> Result<()> {
    source.seek(SeekFrom::Start(0))?;
    destination.seek(SeekFrom::Start(0))?;
    let mut left = vec![0; BUFFER_SIZE];
    let mut right = vec![0; BUFFER_SIZE];
    loop {
        shutdown::check()?;
        let n = source.read(&mut left)?;
        if n == 0 {
            ensure!(
                destination.read(&mut right[..1])? == 0,
                "copy is longer than the source"
            );
            return Ok(());
        }
        destination
            .read_exact(&mut right[..n])
            .context("copy is shorter than the source")?;
        ensure!(
            left[..n] == right[..n],
            "copied bytes differ from the source"
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempfile;

    #[test]
    fn copies_empty_small_and_multi_buffer_files() -> Result<()> {
        let _guard = shutdown::test_guard();
        let temp = tempfile::tempdir()?;
        for size in [0, 11, BUFFER_SIZE, BUFFER_SIZE * 2 + 123] {
            let src = temp.path().join(format!("src{size}"));
            let bytes: Vec<u8> = (0..size).map(|n| (n % 251) as u8).collect();
            fs::write(&src, &bytes)?;
            let mut destination = tempfile()?;
            let (snapshot, meta) = copy_file(&src, &mut destination)?;
            assert_eq!((snapshot.len, meta.len()), (size as u64, size as u64));
            destination.rewind()?;
            let mut copied = Vec::new();
            destination.read_to_end(&mut copied)?;
            assert_eq!(copied, bytes);
        }
        Ok(())
    }

    #[test]
    fn verification_detects_same_length_and_length_differences() -> Result<()> {
        let _guard = shutdown::test_guard();
        let cases: [(&[u8], &[u8], bool); 4] = [
            (b"abc", b"abc", true),
            (b"abc", b"abd", false),
            (b"abc", b"abcd", false),
            (b"abcd", b"abc", false),
        ];
        for (left, right, equal) in cases {
            let (mut a, mut b) = (tempfile()?, tempfile()?);
            a.write_all(left)?;
            b.write_all(right)?;
            assert_eq!(
                verify_equal(&mut a, &mut b).is_ok(),
                equal,
                "{left:?} vs {right:?}"
            );
        }
        Ok(())
    }

    #[test]
    fn cancellation_stops_a_copy() -> Result<()> {
        let _guard = shutdown::test_guard();
        let temp = tempfile::tempdir()?;
        let src = temp.path().join("src");
        fs::write(&src, b"data")?;
        shutdown::request();
        let error = copy_file(&src, &mut tempfile()?).unwrap_err();
        assert_eq!(crate::errors::error_code(&error), "interrupted");
        Ok(())
    }

    #[test]
    fn snapshots_detect_content_and_identity_changes() -> Result<()> {
        let temp = tempfile::tempdir()?;
        let path = temp.path().join("file");
        fs::write(&path, b"one")?;
        let snapshot = Snapshot::of(&File::open(&path)?)?;
        assert!(snapshot.check(&File::open(&path)?, &path).is_ok());
        fs::write(&path, b"three")?;
        assert!(snapshot.check(&File::open(&path)?, &path).is_err());
        let snapshot = Snapshot::of(&File::open(&path)?)?;
        let other = temp.path().join("other");
        fs::write(&other, b"three")?;
        let replacement = Snapshot::of(&File::open(&other)?)?;
        assert_ne!(snapshot.identity, replacement.identity);
        Ok(())
    }
}
