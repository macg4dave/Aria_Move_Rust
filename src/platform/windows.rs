//! Native primitives for Windows. POSIX modes have no equivalent here; only
//! the read-only attribute is meaningful.
use std::fs::{self, DirBuilder, File, Metadata, OpenOptions};
use std::io;
use std::os::windows::ffi::OsStrExt;
use std::os::windows::fs::{MetadataExt, OpenOptionsExt};
use std::path::Path;
use windows_sys::Win32::Foundation::{ERROR_SHARING_VIOLATION, GENERIC_READ, GENERIC_WRITE};
use windows_sys::Win32::Storage::FileSystem::{
    BY_HANDLE_FILE_INFORMATION, DELETE, FILE_ATTRIBUTE_HIDDEN, FILE_ATTRIBUTE_REPARSE_POINT,
    FILE_FLAG_BACKUP_SEMANTICS, FILE_FLAG_DELETE_ON_CLOSE, FILE_FLAG_OPEN_REPARSE_POINT,
    FILE_READ_ATTRIBUTES, FILE_SHARE_READ, GetDiskFreeSpaceExW, GetFileInformationByHandle,
    MOVEFILE_WRITE_THROUGH, MoveFileExW,
};

/// NUL-terminated UTF-16, rejecting interior NULs that would truncate the path.
fn wide(path: &Path) -> io::Result<Vec<u16>> {
    let mut wide: Vec<u16> = path.as_os_str().encode_wide().collect();
    if wide.contains(&0) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "path contains an interior NUL character",
        ));
    }
    wide.push(0);
    Ok(wide)
}

/// Symlinks, junctions and every other reparse-point type.
pub(crate) fn is_link(meta: &Metadata) -> bool {
    meta.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
}

pub(crate) fn file_identity(file: &File, _meta: &Metadata) -> io::Result<(u64, u64)> {
    use std::os::windows::io::AsRawHandle;
    let mut info = std::mem::MaybeUninit::<BY_HANDLE_FILE_INFORMATION>::uninit();
    // SAFETY: `file` owns a valid handle and `info` is writable storage for the result.
    if unsafe { GetFileInformationByHandle(file.as_raw_handle(), info.as_mut_ptr()) } == 0 {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: the successful call initialized `info`.
    let info = unsafe { info.assume_init() };
    Ok((
        u64::from(info.dwVolumeSerialNumber),
        (u64::from(info.nFileIndexHigh) << 32) | u64::from(info.nFileIndexLow),
    ))
}

pub(crate) fn free_space(path: &Path) -> io::Result<u64> {
    let path = wide(path)?;
    let mut available = 0u64;
    // SAFETY: `path` is NUL-terminated; the output pointer is valid and the
    // optional total counters are null.
    let ok = unsafe {
        GetDiskFreeSpaceExW(
            path.as_ptr(),
            &mut available,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
        )
    };
    if ok == 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(available)
}

/// Publication uses `MOVEFILE_WRITE_THROUGH`; directory handles cannot be flushed.
pub(crate) fn sync_directory(_path: &Path) -> io::Result<()> {
    Ok(())
}

/// Hold an exclusive, hidden, delete-on-close sidecar inside the directory.
pub(crate) fn try_lock_directory(dir: &Path) -> io::Result<Option<File>> {
    let path = dir.join(".aria_move.dir.lock");
    super::reject_symlinks(&path)?;
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .access_mode(GENERIC_READ | GENERIC_WRITE | DELETE)
        .share_mode(0)
        .custom_flags(
            FILE_ATTRIBUTE_HIDDEN | FILE_FLAG_DELETE_ON_CLOSE | FILE_FLAG_OPEN_REPARSE_POINT,
        )
        .open(&path);
    match file {
        Ok(file) if is_link(&file.metadata()?) => Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "lock file is a reparse point",
        )),
        Ok(file) => Ok(Some(file)),
        Err(e) if e.raw_os_error() == Some(ERROR_SHARING_VIOLATION as i32) => Ok(None),
        Err(e) => Err(e),
    }
}

pub(super) fn configure_read(options: &mut OpenOptions) {
    options
        .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT)
        .share_mode(FILE_SHARE_READ);
}

pub(super) fn configure_directory(options: &mut OpenOptions) {
    options
        .access_mode(FILE_READ_ATTRIBUTES)
        .custom_flags(FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT);
}

pub(super) fn configure_log(options: &mut OpenOptions) {
    options.custom_flags(FILE_FLAG_OPEN_REPARSE_POINT);
}

pub(super) fn configure_private_dir(_builder: &mut DirBuilder) {}

/// `DeleteFile` refuses read-only files; the attribute is cleared first.
// The lint warns about Unix world-writable modes; on Windows this only clears
// FILE_ATTRIBUTE_READONLY on a file that is about to be deleted.
#[expect(clippy::permissions_set_readonly_false)]
pub(super) fn prepare_removal(path: &Path) -> io::Result<()> {
    let mut permissions = fs::symlink_metadata(path)?.permissions();
    if permissions.readonly() {
        permissions.set_readonly(false);
        fs::set_permissions(path, permissions)?;
    }
    Ok(())
}

pub(super) fn rename_noreplace(src: &Path, dst: &Path) -> io::Result<()> {
    let (from, to) = (wide(src)?, wide(dst)?);
    // SAFETY: both buffers are NUL-terminated and outlive the call. Without
    // MOVEFILE_REPLACE_EXISTING or MOVEFILE_COPY_ALLOWED an existing destination
    // fails and a cross-volume move reports ERROR_NOT_SAME_DEVICE.
    if unsafe { MoveFileExW(from.as_ptr(), to.as_ptr(), MOVEFILE_WRITE_THROUGH) } == 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lock_sidecar_is_hidden_and_deleted_on_release() {
        let temp = tempfile::tempdir().unwrap();
        let sidecar = temp.path().join(".aria_move.dir.lock");
        let lock = try_lock_directory(temp.path()).unwrap().unwrap();
        let attributes = fs::metadata(&sidecar).unwrap().file_attributes();
        assert_ne!(attributes & FILE_ATTRIBUTE_HIDDEN, 0);
        drop(lock);
        assert!(!sidecar.exists());
    }

    #[test]
    fn interior_nul_is_rejected_before_any_native_call() {
        let error = rename_noreplace(Path::new("a\0b"), Path::new("c")).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
    }
}
