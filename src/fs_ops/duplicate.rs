pub enum OnDuplicate { Skip, Overwrite, RenameWithSuffix }

pub fn resolve_destination(dst_dir: &Path, name: &OsStr, policy: OnDuplicate) -> PathBuf {
    // generate unique name if needed when policy is RenameWithSuffix
    // ...
}