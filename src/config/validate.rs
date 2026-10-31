//! Canonical configuration validation, with preparation separated from dry runs.
use super::{Config, paths};
use anyhow::{Result, bail};
use std::path::Path;

/// Validate disjoint paths before preparing directories. Dry runs never create them.
pub fn validate_and_normalize(cfg: &mut Config) -> Result<()> {
    for path in [&cfg.download_base, &cfg.completed_base] {
        paths::reject_symlinks(path)?;
        if path.exists() && !path.is_dir() {
            bail!("'{}' exists but is not a directory", path.display());
        }
        if path == Path::new("/path/to/incoming") || path == Path::new("/path/to/completed") {
            bail!(
                "Refusing to create placeholder default path '{}'; edit config.xml",
                path.display()
            );
        }
    }
    let download = paths::normalize_missing(&cfg.download_base)?;
    let completed = paths::normalize_missing(&cfg.completed_base)?;
    if download == completed {
        bail!(
            "download_base and completed_base resolve to the same path: '{}'",
            download.display()
        );
    }
    if download.starts_with(&completed) {
        bail!(
            "download_base '{}' must not be inside completed_base '{}'",
            download.display(),
            completed.display()
        );
    }
    if completed.starts_with(&download) {
        bail!(
            "completed_base '{}' must not be inside download_base '{}'",
            completed.display(),
            download.display()
        );
    }
    if !cfg.dry_run {
        paths::create_directory(&download)?;
        paths::create_directory(&completed)?;
    }
    cfg.download_base = download;
    cfg.completed_base = completed;
    Ok(())
}
