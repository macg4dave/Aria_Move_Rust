//! Bounded, cancellable transfer and byte-for-byte verification.
use std::fs::File;
use std::io::{self, Read, Seek, SeekFrom, Write};

const BUFFER_SIZE: usize = 1024 * 1024;

pub(super) fn copy_verified(source: &mut File, destination: &mut File) -> crate::AMResult<u64> {
    source.seek(SeekFrom::Start(0))?;
    let mut buffer = vec![0; BUFFER_SIZE];
    let mut total = 0u64;
    loop {
        crate::shutdown::check()?;
        let count = match source.read(&mut buffer) {
            Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
            other => other?,
        };
        if count == 0 {
            break;
        }
        destination.write_all(&buffer[..count])?;
        total = total
            .checked_add(count as u64)
            .ok_or_else(|| io::Error::other("file too large"))?;
    }
    destination.sync_all()?;
    verify_equal(source, destination)?;
    Ok(total)
}

/// Check both bytes and EOF. Used immediately before source deletion as well as
/// staging, so same-length changes are not mistaken for a completed transfer.
pub(super) fn verify_equal(source: &mut File, destination: &mut File) -> crate::AMResult<()> {
    source.seek(SeekFrom::Start(0))?;
    destination.seek(SeekFrom::Start(0))?;
    let mut left = vec![0; BUFFER_SIZE];
    let mut right = vec![0; BUFFER_SIZE];
    loop {
        crate::shutdown::check()?;
        let n = source.read(&mut left)?;
        if n == 0 {
            anyhow::ensure!(
                destination.read(&mut right[..1])? == 0,
                "copy has extra data"
            );
            return Ok(());
        }
        destination.read_exact(&mut right[..n])?;
        anyhow::ensure!(
            left[..n] == right[..n],
            "source changed or copied data differs"
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempfile;

    #[test]
    fn verifies_empty_small_and_buffer_boundaries() -> crate::AMResult<()> {
        let _guard = crate::shutdown::test_guard();
        for size in [0, 11, BUFFER_SIZE * 2 + 123] {
            let mut source = tempfile()?;
            let bytes: Vec<_> = (0..size).map(|n| (n % 251) as u8).collect();
            source.write_all(&bytes)?;
            let mut destination = tempfile()?;
            assert_eq!(copy_verified(&mut source, &mut destination)?, size as u64);
            destination.rewind()?;
            let mut actual = Vec::new();
            destination.read_to_end(&mut actual)?;
            assert_eq!(actual, bytes);
        }
        Ok(())
    }

    #[test]
    fn detects_equal_size_corruption_and_cancellation() -> crate::AMResult<()> {
        let _guard = crate::shutdown::test_guard();
        let mut source = tempfile()?;
        let mut destination = tempfile()?;
        source.write_all(b"abc")?;
        destination.write_all(b"xyz")?;
        assert!(verify_equal(&mut source, &mut destination).is_err());
        crate::shutdown::request();
        let error = copy_verified(&mut source, &mut destination).unwrap_err();
        crate::shutdown::reset();
        assert_eq!(
            error.downcast_ref::<crate::AriaMoveError>(),
            Some(&crate::AriaMoveError::Interrupted)
        );
        Ok(())
    }
}
