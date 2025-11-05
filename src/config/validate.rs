use anyhow::{bail, Context, Result};
use std::fs;

#[cfg(unix)]
use libc;

use tracing::{debug, error, info};

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
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let _ =
                    fs::set_permissions(&self.completed_base, fs::Permissions::from_mode(0o700));
            }
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

        // Unix-specific ownership & permission checks
        #[cfg(unix)]
        {
            use std::os::unix::fs::{MetadataExt, PermissionsExt};

            // download_base permissions/ownership
            let db_meta = fs::metadata(&self.download_base).with_context(|| {
                format!(
                    "Failed to stat download base '{}'",
                    self.download_base.display()
                )
            })?;
            if db_meta.permissions().mode() & 0o022 != 0 {
                bail!("Download base '{}' is group/world-writable; refuse to operate on insecure directory", self.download_base.display());
            }
            if db_meta.uid() != unsafe { libc::geteuid() } {
                bail!(
                    "Download base '{}' is not owned by current user (uid {})",
                    self.download_base.display(),
                    unsafe { libc::geteuid() }
                );
            }

            // completed_base permissions/ownership
            let cb_meta = fs::metadata(&self.completed_base).with_context(|| {
                format!(
                    "Failed to stat completed base '{}'",
                    self.completed_base.display()
                )
            })?;
            if cb_meta.permissions().mode() & 0o022 != 0 {
                bail!("Completed base '{}' is group/world-writable; refuse to operate on insecure directory", self.completed_base.display());
            }
            if cb_meta.uid() != unsafe { libc::geteuid() } {
                bail!(
                    "Completed base '{}' is not owned by current user (uid {})",
                    self.completed_base.display(),
                    unsafe { libc::geteuid() }
                );
            }
        }

        // Windows: minimal checks + warning (full ACL/SID checks not implemented)
        #[cfg(windows)]
        {
            use std::os::windows::fs::MetadataExt;
            const FILE_ATTRIBUTE_READONLY: u32 = 0x0000_0001;

            for (label, path) in [
                ("download_base", &self.download_base),
                ("completed_base", &self.completed_base),
            ] {
                if let Ok(meta) = fs::metadata(path) {
                    let attrs = meta.file_attributes();
                    if attrs & FILE_ATTRIBUTE_READONLY != 0 {
                        bail!(
                            "{} '{}' has the READONLY attribute set; cannot write",
                            label,
                            path.display()
                        );
                    }
                }
            }

            tracing::warn!(
                "Windows ACL validation not implemented. Ensure:\n\
                 1. Directories are owned by the current user\n\
                 2. 'Everyone' does NOT have Write permissions\n\
                 3. Use `icacls <path>` to verify ACLs manually"
            );
        }

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