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
    #[deprecated(since = "0.2.0", note = "Use config::validate_and_normalize instead; this will be removed in a future release.")]
    pub fn validate(&self) -> Result<()> {
        // Deprecated path: delegate to config::validate_and_normalize for unified logic.
        // Keep extended readability/writability and platform security checks for now.
        use crate::config::validate_and_normalize; // avoid recursion
        let mut owned = self.clone();
        validate_and_normalize(&mut owned)?;

        // Perform readability/writability checks similar to legacy behavior.
        ensure_dir_exists_and_is_dir(&owned.download_base, "download_base")?;
        ensure_readable(&owned.download_base, "download_base")?;
        ensure_dir_is_or_create(&owned.completed_base, "completed_base")?; // safe if already exists
        ensure_writable(&owned.completed_base, "completed_base")?;
        ensure_secure_directory(&owned.download_base, "download_base")?;
        ensure_secure_directory(&owned.completed_base, "completed_base")?;
        info!(
            "Config validated: download='{}' completed='{}' log_file='{}'",
            owned.download_base.display(),
            owned.completed_base.display(),
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
