use anyhow::anyhow;
use std::io;
use std::path::Path;

#[cfg(unix)]
use std::os::unix::ffi::OsStrExt;
#[cfg(windows)]
use std::os::windows::ffi::OsStrExt;

pub(super) fn format_bytes(n: u64) -> String {
    const KB: f64 = 1024.0;
    const MB: f64 = KB * 1024.0;
    const GB: f64 = MB * 1024.0;
    let f = n as f64;
    if f >= GB {
        format!("{:.1} GiB", f / GB)
    } else if f >= MB {
        format!("{:.1} MiB", f / MB)
    } else if f >= KB {
        format!("{:.1} KiB", f / KB)
    } else {
        format!("{} B", n)
    }
}

pub(super) fn ensure_space_for_copy(dst_dir: &Path, required: u64) -> anyhow::Result<()> {
    let free = free_space_bytes(dst_dir)?;
    let cushion: u64 = 4 * 1024 * 1024;
    if free < required.saturating_add(cushion) {
        return Err(anyhow!(
            "not enough free space in '{}': need ~{}, free {}",
            dst_dir.display(),
            format_bytes(required),
            format_bytes(free)
        ));
    }
    Ok(())
}

#[cfg(unix)]
pub(super) fn free_space_bytes(path: &Path) -> io::Result<u64> {
    let mut s: libc::statvfs = unsafe { std::mem::zeroed() };
    let cpath = std::ffi::CString::new(path.as_os_str().as_bytes())
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "path contains NUL"))?;
    let rc = unsafe { libc::statvfs(cpath.as_ptr(), &mut s) };
    if rc != 0 {
        return Err(io::Error::last_os_error());
    }
    Ok((s.f_bavail as u64).saturating_mul(s.f_frsize as u64))
}

#[cfg(windows)]
pub(super) fn free_space_bytes(path: &Path) -> io::Result<u64> {
    use std::iter::once;
    use windows_sys::Win32::Storage::FileSystem::GetDiskFreeSpaceExW;
    let wide: Vec<u16> = path.as_os_str().encode_wide().chain(once(0)).collect();
    let mut free_avail: u64 = 0;
    let mut _total: u64 = 0;
    let mut _total_free: u64 = 0;
    let ok = unsafe {
        GetDiskFreeSpaceExW(
            wide.as_ptr(),
            &mut free_avail as *mut u64,
            &mut _total as *mut u64,
            &mut _total_free as *mut u64,
        )
    };
    if ok == 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(free_avail)
}