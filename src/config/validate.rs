//! Config validation logic.
//! Verifies directory existence, readability/writability, disjoint paths, and platform-specific security checks.

use anyhow::{bail, Context, Result};
use std::fs;
use std::path::Path;
use tracing::{debug, error, info};

use crate::platform::ensure_secure_directory;
use crate::utils::is_writable_probe;

use super::types::Config;

impl Config {
    /// Validate existence, readability/writability and canonical paths.
    pub fn validate(&self) -> Result<()> {
        let db = &self.download_base;
        let cb = &self.completed_base;

        // 1) Download base: must exist, be a directory, and be readable.
        ensure_dir_exists_and_is_dir(db, "download_base")?;
        ensure_readable(db, "download_base")?;

        // 2) Completed base: must be a directory; create if missing; ensure writable.
        ensure_dir_is_or_create(cb, "completed_base")?;
        ensure_writable(cb, "completed_base")?;

        // 3) Resolve symlinks and ensure the bases are disjoint (neither contains the other).
        let db_real = fs::canonicalize(db).unwrap_or_else(|_| db.clone());
        let cb_real = fs::canonicalize(cb).unwrap_or_else(|_| cb.clone());

        if db_real == cb_real {
            bail!(
                "download_base and completed_base resolve to the same path: '{}'",
                db_real.display()
            );
        }
        if db_real.starts_with(&cb_real) {
            bail!(
                "download_base '{}' must not be inside completed_base '{}'",
                db_real.display(),
                cb_real.display()
            );
        }
        if cb_real.starts_with(&db_real) {
            bail!(
                "completed_base '{}' must not be inside download_base '{}'",
                cb_real.display(),
                db_real.display()
            );
        }

        // 4) Platform-specific directory security checks (perms, ownership, etc).
        ensure_secure_directory(db, "download_base")?;
        ensure_secure_directory(cb, "completed_base")?;

        info!(
            "Config validated: download='{}' completed='{}' log_file='{}'",
            db.display(),
            cb.display(),
            self.log_file
                .as_ref()
                .map(|p| p.display().to_string())
                .unwrap_or_else(|| "<none>".into())
        );
        Ok(())
    }
}

/// Ensure path exists and is a directory; emit clear errors with path context.
fn ensure_dir_exists_and_is_dir(path: &Path, name: &str) -> Result<()> {
    if !path.exists() {
        error!("{name} does not exist: {}", path.display());
        bail!("{name} does not exist: {}", path.display());
    }
    if !path.is_dir() {
        error!("{name} is not a directory: {}", path.display());
        bail!("{name} is not a directory: {}", path.display());
    }
    Ok(())
}

/// Ensure directory is readable by attempting to open its entries.
fn ensure_readable(path: &Path, name: &str) -> Result<()> {
    fs::read_dir(path).with_context(|| {
        format!("Cannot read {name} directory '{}'; check permissions", path.display())
    })?;
    debug!("{name} readable: {}", path.display());
    Ok(())
}

/// Ensure directory exists (create if missing). If exists, it must be a directory.
fn ensure_dir_is_or_create(path: &Path, name: &str) -> Result<()> {
    if path.exists() {
        if !path.is_dir() {
            error!("{name} exists but isn't a directory: {}", path.display());
            bail!("{name} exists but isn't a directory: {}", path.display());
        }
    } else {
        fs::create_dir_all(path).with_context(|| {
            format!("Failed to create {name} directory '{}'", path.display())
        })?;
        info!("Created {name} directory: {}", path.display());
    }
    Ok(())
}

/// Ensure directory is writable using a non-destructive probe file.
fn ensure_writable(path: &Path, name: &str) -> Result<()> {
    is_writable_probe(path).with_context(|| {
        format!("Cannot write to {name} '{}'; check permissions", path.display())
    })?;
    debug!("{name} writable: {}", path.display());
    Ok(())
}
