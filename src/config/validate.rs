//! Config validation logic.
//! Verifies directory existence, readability/writability, disjoint paths, and platform-specific security checks.

use anyhow::{bail, Context, Result};
use std::fs;
use tracing::{debug, error, info};

use crate::platform::ensure_secure_directory;
use crate::utils::is_writable_probe;

use super::types::Config;

impl Config {
    /// Validate existence, readability/writability and canonical paths.
    pub fn validate(&self) -> Result<()> {
        if !self.download_base.exists() {
            error!(
                "Download base does not exist: {}",
                self.download_base.display()
            );
            bail!(
                "Download base does not exist: {}",
                self.download_base.display()
            );
        }
        if !self.download_base.is_dir() {
            error!(
                "Download base is not a directory: {}",
                self.download_base.display()
            );
            bail!(
                "Download base is not a directory: {}",
                self.download_base.display()
            );
        }

        fs::read_dir(&self.download_base).with_context(|| {
            format!(
                "Cannot read download base directory '{}'; check permissions",
                self.download_base.display()
            )
        })?;
        debug!("Download base readable: {}", self.download_base.display());

        if self.completed_base.exists() && !self.completed_base.is_dir() {
            error!(
                "Completed base exists but isn't a directory: {}",
                self.completed_base.display()
            );
            bail!(
                "Completed base exists but isn't a directory: {}",
                self.completed_base.display()
            );
        }
        if !self.completed_base.exists() {
            fs::create_dir_all(&self.completed_base).with_context(|| {
                format!(
                    "Failed to create completed base directory '{}'",
                    self.completed_base.display()
                )
            })?;
            info!(
                "Created completed base directory: {}",
                self.completed_base.display()
            );
        }

        // writability probe
        is_writable_probe(&self.completed_base).with_context(|| {
            format!(
                "Cannot write to completed base '{}'; check permissions",
                self.completed_base.display()
            )
        })?;
        debug!("Completed base writable: {}", self.completed_base.display());

        // Resolve symlinks and ensure the bases are disjoint (neither contains the other).
        let db_real =
            fs::canonicalize(&self.download_base).unwrap_or_else(|_| self.download_base.clone());
        let cb_real =
            fs::canonicalize(&self.completed_base).unwrap_or_else(|_| self.completed_base.clone());

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

        // Platform-specific directory security checks
        ensure_secure_directory(&self.download_base, "download_base")?;
        ensure_secure_directory(&self.completed_base, "completed_base")?;

        info!(
            "Config validated: download='{}' completed='{}' log_file='{}'",
            self.download_base.display(),
            self.completed_base.display(),
            self.log_file
                .as_ref()
                .map(|p| p.display().to_string())
                .unwrap_or_else(|| "<none>".into())
        );
        Ok(())
    }
}
