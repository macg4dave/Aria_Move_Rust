//! Config module (modularized).
//! Provides configuration types, default paths, XML loading, and validation.
//! Re-exports preserve the previous public API for external callers.

pub mod paths;
pub mod types;
pub mod xml;

use anyhow::{anyhow, Context, Result};
use std::fs;
use std::io::{self, Write};
use std::path::{Component, Path, PathBuf};

pub use types::{Config, LogLevel};
pub use paths::{default_config_path, default_log_path};

// --- existing/public load_or_init / validate_and_normalize functions remain ---
#[derive(Debug)]
pub enum LoadResult {
    Loaded(types::Config, PathBuf),
    CreatedTemplate(PathBuf),
}

/// Load config from default path (or ARIA_MOVE_CONFIG). If missing, write a secure template and return CreatedTemplate.
pub fn load_or_init() -> Result<LoadResult> {
    let path = default_config_path()?;
    if path.exists() {
        return Ok(LoadResult::Loaded(types::Config::default(), path));
    }

    if let Some(parent) = path.parent() {
        create_secure_dir_all(parent)?;
    }
    write_template(&path)?;
    Ok(LoadResult::CreatedTemplate(path))
}

/// Validate and normalize config paths:
/// - Ensure directories exist (create if missing) with safe perms
/// - Reject symlink ancestors (Unix)
/// - Canonicalize final paths back into cfg
/// - Ensure download_base and completed_base are disjoint (neither equal nor nested)
pub fn validate_and_normalize(cfg: &mut types::Config) -> Result<()> {
    ensure_safe_dir(&cfg.download_base)
        .with_context(|| format!("download_base invalid: {}", cfg.download_base.display()))?;
    cfg.download_base = canonicalize_best_effort(&cfg.download_base)?;

    ensure_safe_dir(&cfg.completed_base)
        .with_context(|| format!("completed_base invalid: {}", cfg.completed_base.display()))?;
    cfg.completed_base = canonicalize_best_effort(&cfg.completed_base)?;

    // Disjointness checks after canonicalization
    if cfg.download_base == cfg.completed_base {
        return Err(anyhow!(
            "download_base and completed_base resolve to the same path: '{}'",
            cfg.download_base.display()
        ));
    }
    if cfg.download_base.starts_with(&cfg.completed_base) {
        return Err(anyhow!(
            "download_base '{}' must not be inside completed_base '{}'",
            cfg.download_base.display(),
            cfg.completed_base.display()
        ));
    }
    if cfg.completed_base.starts_with(&cfg.download_base) {
        return Err(anyhow!(
            "completed_base '{}' must not be inside download_base '{}'",
            cfg.completed_base.display(),
            cfg.download_base.display()
        ));
    }
    Ok(())
}

/// Ensure a default config exists (create template if missing).
/// Returns the path that was created or the existing config path.
pub fn ensure_default_config_exists() -> Result<PathBuf> {
    match load_or_init()? {
        LoadResult::Loaded(_, p) => Ok(p),
        LoadResult::CreatedTemplate(p) => Ok(p),
    }
}

/// Public wrapper that checks whether a path has a symlink ancestor.
/// Calls the internal has_symlink_ancestor implementation.
pub fn path_has_symlink_ancestor(path: &Path) -> io::Result<bool> {
    has_symlink_ancestor(path)
}

fn write_template(path: &Path) -> io::Result<()> {
    let template = r#"<!-- aria_move config (XML) -->
<!-- Edit the paths below and rerun aria_move -->
<config>
  <!-- Where partial/new downloads appear -->
  <download_base>/path/to/incoming</download_base>
  <!-- Final destination for completed items -->
  <completed_base>/path/to/completed</completed_base>

  <!-- quiet | normal | info | debug -->
  <log_level>normal</log_level>
  <!-- Optional: full path to log file -->
  <log_file></log_file>

  <!-- Preserve permissions and mtime when moving (slower) -->
  <preserve_metadata>false</preserve_metadata>
  <!-- Recency window (seconds) for auto-resolving recent file) -->
  <recent_window_seconds>300</recent_window_seconds>
</config>
"#;

    let mut f = fs::File::create(path)?;
    f.write_all(template.as_bytes())?;
    f.sync_all()?;
    Ok(())
}

/// Ensure path exists as a directory, reject symlink ancestors (Unix), and enforce safe perms.
fn ensure_safe_dir(dir: &Path) -> Result<()> {
    if !dir.exists() {
        create_secure_dir_all(dir).with_context(|| format!("create directory '{}'", dir.display()))?;
    } else if !dir.is_dir() {
        return Err(anyhow!("'{}' exists but is not a directory", dir.display()));
    }

    // Reject symlink ancestors (Unix)
    #[cfg(unix)]
    {
        if has_symlink_ancestor(dir)? {
            return Err(anyhow!(
                "refusing directory under a symlinked ancestor: {}",
                dir.display()
            ));
        }
        use std::os::unix::fs::PermissionsExt;
        let meta = fs::metadata(dir)?;
        let mode = meta.permissions().mode() & 0o777;
        if mode & 0o022 != 0 {
            return Err(anyhow!(
                "unsafe permissions {:o} on {}; group/world-writable not allowed",
                mode,
                dir.display()
            ));
        }
    }

    Ok(())
}

fn create_secure_dir_all(dir: &Path) -> io::Result<()> {
    fs::create_dir_all(dir)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let perms = fs::Permissions::from_mode(0o700);
        fs::set_permissions(dir, perms)?;
    }
    Ok(())
}

fn canonicalize_best_effort(path: &Path) -> Result<PathBuf> {
    match dunce::canonicalize(path) {
        Ok(p) => Ok(p),
        Err(e) => Err(anyhow!("canonicalize {} failed: {e}", path.display())),
    }
}

#[cfg(unix)]
fn has_symlink_ancestor(path: &Path) -> io::Result<bool> {
    use std::fs::symlink_metadata;
    // Build up from root, lstat each part; treat non-existent parts as safe.
    let mut cur = PathBuf::new();
    for comp in path.components() {
        match comp {
            Component::CurDir => continue,
            Component::ParentDir => cur.push(".."),
            Component::RootDir | Component::Prefix(_) => cur.push(comp),
            Component::Normal(p) => {
                cur.push(p);
                if cur.exists() {
                    let meta = symlink_metadata(&cur)?;
                    if meta.file_type().is_symlink() {
                        return Ok(true);
                    }
                }
            }
        }
    }
    Ok(false)
}

#[cfg(windows)]
fn has_symlink_ancestor(_path: &Path) -> io::Result<bool> {
    // Not enforced on Windows in this build.
    Ok(false)
}

/// Default download base when no config or CLI override is provided.
/// Historically some users used `/mnt/World` on specific systems; adjust via config or CLI.
pub const DOWNLOAD_BASE_DEFAULT: &str = "/mnt/World";

/// Default completed base directory used when no config or CLI override is provided.
pub const COMPLETED_BASE_DEFAULT: &str = "/mnt/World/completed";
