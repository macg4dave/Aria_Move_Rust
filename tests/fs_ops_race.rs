#![cfg(unix)]
use std::fs;
use std::os::unix::fs::symlink;
#[test]
fn existing_and_dangling_destination_links_are_never_replaced() -> anyhow::Result<()> {
    let td = tempfile::tempdir()?;
    let root = dunce::canonicalize(td.path())?;
    let src = root.join("source");
    let outside = root.join("outside");
    let dest = root.join("destination");
    fs::write(&src, b"source")?;
    fs::write(&outside, b"outside")?;
    for target in [&outside, &root.join("missing")] {
        symlink(target, &dest)?;
        assert!(aria_move::safe_copy_and_rename(&src, &dest).is_err());
        assert!(fs::symlink_metadata(&dest)?.file_type().is_symlink());
        assert_eq!(fs::read(&outside)?, b"outside");
        assert_eq!(fs::read(&src)?, b"source");
        fs::remove_file(&dest)?;
    }
    Ok(())
}
