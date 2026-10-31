//! Copy to exclusively owned staging, verify, then publish without replacement.
//! Legacy resume files are untrusted: they are neither modified nor published.
use anyhow::{Context, Result, ensure};
use std::fs::{self, File, Metadata};
use std::path::Path;

/// A file snapshot sufficient to reject identity, size and timestamp changes.
#[derive(Debug, PartialEq, Eq)]
pub(super) struct Snapshot {
    len: u64,
    modified: std::time::SystemTime,
    identity: (u64, u64),
}

impl Snapshot {
    pub(super) fn read(file: &File) -> Result<Self> {
        let meta = file.metadata()?;
        #[cfg(unix)]
        let identity = {
            use std::os::unix::fs::MetadataExt;
            (meta.dev(), meta.ino())
        };
        #[cfg(windows)]
        let identity = {
            use std::os::windows::io::AsRawHandle;
            use windows_sys::Win32::Storage::FileSystem::{
                BY_HANDLE_FILE_INFORMATION, GetFileInformationByHandle,
            };
            let mut info = std::mem::MaybeUninit::<BY_HANDLE_FILE_INFORMATION>::uninit();
            // SAFETY: file owns a valid handle and info points to writable storage.
            if unsafe { GetFileInformationByHandle(file.as_raw_handle() as _, info.as_mut_ptr()) }
                == 0
            {
                return Err(std::io::Error::last_os_error().into());
            }
            // SAFETY: the successful call initialized info.
            let info = unsafe { info.assume_init() };
            (
                info.dwVolumeSerialNumber as u64,
                (info.nFileIndexHigh as u64) << 32 | info.nFileIndexLow as u64,
            )
        };
        Ok(Self {
            len: if meta.is_dir() { 0 } else { meta.len() },
            modified: if meta.is_dir() {
                std::time::SystemTime::UNIX_EPOCH
            } else {
                meta.modified()?
            },
            identity,
        })
    }

    pub(super) fn check(&self, file: &File) -> Result<()> {
        ensure!(*self == Self::read(file)?, "source changed during transfer");
        Ok(())
    }
}

/// Copy a regular source into a caller-owned destination handle.
pub(super) fn copy_to_file(
    src: &Path,
    destination: &mut File,
    preserve: bool,
    permissions: bool,
    dest: &Path,
) -> Result<(Snapshot, Metadata)> {
    let mut source = crate::platform::open_regular_read(src)
        .with_context(|| format!("open source {}", src.display()))?;
    let snapshot = Snapshot::read(&source)?;
    let meta = source.metadata()?;
    let written = super::io_copy::copy_verified(&mut source, destination)?;
    ensure!(written == meta.len(), "source size changed during copy");
    snapshot.check(&source)?;
    apply_metadata(src, dest, &meta, preserve, permissions)?;
    destination.sync_all()?;
    Ok((snapshot, meta))
}

pub(super) fn apply_metadata(
    src: &Path,
    dest: &Path,
    meta: &Metadata,
    preserve: bool,
    permissions: bool,
) -> Result<()> {
    if preserve {
        super::metadata::preserve_metadata(dest, meta)?;
        super::metadata::preserve_xattrs(src, dest)?;
    } else if permissions {
        super::metadata::preserve_permissions_only(dest, meta)?;
    }
    Ok(())
}

pub fn safe_copy_and_rename(src: &Path, dest: &Path) -> Result<()> {
    safe_copy_and_rename_with_metadata(src, dest, false)
}

/// Publish verified source bytes; existing destinations always produce an error.
/// Metadata is best effort, diagnosed by the metadata helpers. Source is retained.
pub fn safe_copy_and_rename_with_metadata(src: &Path, dest: &Path, preserve: bool) -> Result<()> {
    copy_with(src, dest, preserve, || Ok(()))
}

fn copy_with(
    src: &Path,
    dest: &Path,
    preserve: bool,
    before_publish: impl FnOnce() -> Result<()>,
) -> Result<()> {
    crate::shutdown::check()?;
    crate::config::paths::reject_symlinks(src)?;
    let parent = dest
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    crate::config::paths::create_directory(parent)?;
    if fs::symlink_metadata(dest).is_ok() {
        return Err(
            std::io::Error::new(std::io::ErrorKind::AlreadyExists, "destination exists").into(),
        );
    }
    let mut stage = tempfile::Builder::new()
        .prefix(".aria_move.copy.")
        .tempfile_in(parent)?;
    let stage_path = stage.path().to_path_buf();
    let (snapshot, meta) = copy_to_file(src, stage.as_file_mut(), false, false, &stage_path)?;
    before_publish()?;
    crate::shutdown::check()?;
    let mut source = crate::platform::open_regular_read(src)?;
    snapshot.check(&source)?;
    super::io_copy::verify_equal(&mut source, stage.as_file_mut())?;
    snapshot.check(&source)?;
    apply_metadata(src, &stage_path, &meta, preserve, false)?;
    stage.as_file().sync_all()?;
    crate::platform::rename_noreplace(stage.path(), dest)?;
    crate::platform::sync_directory(parent)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn publication_collision_and_interrupt_leave_source_and_no_owned_temp() -> Result<()> {
        let _guard = crate::shutdown::test_guard();
        let td = tempfile::tempdir()?;
        let src = td.path().join("source");
        let dest = td.path().join("destination");
        fs::write(&src, b"source")?;
        let result = copy_with(&src, &dest, false, || {
            fs::write(&dest, b"external")?;
            Ok(())
        });
        assert!(result.is_err());
        assert_eq!(fs::read(&src)?, b"source");
        assert_eq!(fs::read(&dest)?, b"external");
        fs::remove_file(&dest)?;
        let result = copy_with(&src, &dest, false, || {
            crate::shutdown::request();
            Ok(())
        });
        crate::shutdown::reset();
        assert_eq!(
            result.unwrap_err().downcast_ref::<crate::AriaMoveError>(),
            Some(&crate::AriaMoveError::Interrupted)
        );
        assert!(!dest.exists());
        assert_eq!(fs::read_dir(td.path())?.count(), 1);
        let result = copy_with(&src, &dest, false, || {
            fs::write(&src, b"change")?;
            Ok(())
        });
        assert!(result.is_err());
        assert!(!dest.exists());
        assert_eq!(fs::read(&src)?, b"change");
        Ok(())
    }
}
