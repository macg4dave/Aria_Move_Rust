use std::fs::{File, OpenOptions};
use std::io;
use std::path::Path;

pub(super) fn copy_streaming(src: &Path, dst: &Path) -> io::Result<u64> {
    let mut r = File::open(src)?;
    let mut w = OpenOptions::new().write(true).create_new(true).open(dst)?;
    let mut buf = vec![0u8; 1024 * 1024];
    let mut total = 0u64;
    loop {
        let n = std::io::Read::read(&mut r, &mut buf)?;
        if n == 0 {
            break;
        }
        std::io::Write::write_all(&mut w, &buf[..n])?;
        total += n as u64;
    }
    std::io::Write::flush(&mut w)?;
    w.sync_all()?;
    Ok(total)
}