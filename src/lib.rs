//! Core library for `aria_move`.
//!
//! Contains the core logic: config loading, path resolution and moves.
//! Keep the library small and ergonomic: a Config type with sensible defaults,
//! a method to validate paths/permissions, and pure functions that perform moves.
use anyhow::{bail, Context, Result};
use rayon::prelude::*;
use std::env;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};
use walkdir::WalkDir;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use tracing::{debug, error, info, warn};

/// Defaults used when no config file is present.
///
/// These are intentionally simple constants — users should override in an XML file
/// or with CLI flags when deploying to different systems.
pub const DOWNLOAD_BASE_DEFAULT: &str = "/mnt/World/incoming";
pub const COMPLETED_BASE_DEFAULT: &str = "/mnt/World/completed";
pub const RECENT_FILE_WINDOW: Duration = Duration::from_secs(5 * 60);

/// Try to load download/completed base and optional log level from an XML config file.
///
/// Search order:
///  - $ARIA_MOVE_CONFIG (explicit)
///  - OS-appropriate default in the user's home dir:
///     - macOS: $HOME/Library/Application Support/aria_move/config.xml
///     - Linux/Unix: $XDG_CONFIG_HOME/aria_move/config.xml or $HOME/.config/aria_move/config.xml
///
/// The parser is intentionally minimal and tolerant: it looks for simple
/// <download_base>, <completed_base> and <log_level> tags. If no useful data
/// is found the function returns None.
fn load_config_from_xml() -> Option<(PathBuf, PathBuf, Option<String>)> {
    let env_path = env::var("ARIA_MOVE_CONFIG").ok().map(PathBuf::from);
    // `or_else(default_config_path)` yields an Option<PathBuf>; handle that explicitly
    let cfg_path = if let Some(p) = env_path.clone().or_else(default_config_path) {
        p
    } else {
        return None;
    };

    // If using the default location and the file doesn't exist, create a template
    // so users get a sensible starting point with conservative permissions.
    if !cfg_path.exists() {
        if env_path.is_none() {
            let _ = create_template_config(&cfg_path);
        }
        return None;
    }

    let content = fs::read_to_string(&cfg_path).ok()?;

    fn extract_tag(s: &str, tag: &str) -> Option<String> {
        let open = format!("<{}>", tag);
        let close = format!("</{}>", tag);
        let start = s.find(&open)?;
        let after = start + open.len();
        let rel_end = s[after..].find(&close)?;
        let val = s[after..after + rel_end].trim().to_string();
        if val.is_empty() {
            None
        } else {
            Some(val)
        }
    }

    let download_base = extract_tag(&content, "download_base").map(PathBuf::from);
    let completed_base = extract_tag(&content, "completed_base").map(PathBuf::from);
    let log_level = extract_tag(&content, "log_level");

    if download_base.is_none() && completed_base.is_none() && log_level.is_none() {
        return None;
    }

    Some((
        download_base.unwrap_or_else(|| PathBuf::from(DOWNLOAD_BASE_DEFAULT)),
        completed_base.unwrap_or_else(|| PathBuf::from(COMPLETED_BASE_DEFAULT)),
        log_level,
    ))
}

/// OS-appropriate default config file path under the user's home directory.
fn default_config_path() -> Option<PathBuf> {
    let home = env::var("HOME").ok()?;
    let home = PathBuf::from(home);

    if cfg!(target_os = "macos") {
        Some(home.join("Library").join("Application Support").join("aria_move").join("config.xml"))
    } else {
        if let Ok(xdg) = env::var("XDG_CONFIG_HOME") {
            Some(PathBuf::from(xdg).join("aria_move").join("config.xml"))
        } else {
            Some(home.join(".config").join("aria_move").join("config.xml"))
        }
    }
}

/// Create parent directory and write a small secure template config file.
///
/// On Unix this will attempt to set conservative permissions:
///  - dir: 0o700
///  - file: 0o600
fn create_template_config(path: &Path) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
        #[cfg(unix)]
        {
            // Best-effort: ignore permission-setting errors so creation still succeeds on weird filesystems.
            let _ = fs::set_permissions(parent, fs::Permissions::from_mode(0o700));
        }
    }

    let content = format!(
        "<config>\n  <download_base>{}</download_base>\n  <completed_base>{}</completed_base>\n  <log_level>info</log_level>\n</config>\n",
        DOWNLOAD_BASE_DEFAULT,
        COMPLETED_BASE_DEFAULT
    );

    fs::write(path, content)?;
    #[cfg(unix)]
    {
        let _ = fs::set_permissions(path, fs::Permissions::from_mode(0o600));
    }

    info!("Created template config at {}", path.display());
    Ok(())
}

/// Configuration holder for base paths, time window and optional log level.
#[derive(Debug, Clone)]
pub struct Config {
    pub download_base: PathBuf,
    pub completed_base: PathBuf,
    pub recent_window: Duration,
    pub log_level: Option<String>,
}

impl Default for Config {
    fn default() -> Self {
        if let Some((db, cb, lvl)) = load_config_from_xml() {
            Self {
                download_base: db,
                completed_base: cb,
                recent_window: RECENT_FILE_WINDOW,
                log_level: lvl,
            }
        } else {
            Self {
                download_base: PathBuf::from(DOWNLOAD_BASE_DEFAULT),
                completed_base: PathBuf::from(COMPLETED_BASE_DEFAULT),
                recent_window: RECENT_FILE_WINDOW,
                log_level: None,
            }
        }
    }
}

impl Config {
    /// Construct config with explicit values.
    pub fn new(download_base: impl Into<PathBuf>, completed_base: impl Into<PathBuf>, recent_window: Duration) -> Self {
        Self {
            download_base: download_base.into(),
            completed_base: completed_base.into(),
            recent_window,
            log_level: None,
        }
    }

    /// Validate configured base paths for sanity and permissions.
    ///
    /// - download_base must exist and be readable.
    /// - completed_base will be created if missing and must be writable.
    /// - download_base and completed_base must not resolve to the same path.
    pub fn validate(&self) -> Result<()> {
        // download_base: existence and directory
        if !self.download_base.exists() {
            error!("Download base does not exist: {}", self.download_base.display());
            bail!("Download base does not exist: {}", self.download_base.display());
        }
        if !self.download_base.is_dir() {
            error!("Download base is not a directory: {}", self.download_base.display());
            bail!("Download base is not a directory: {}", self.download_base.display());
        }

        // readability probe
        fs::read_dir(&self.download_base).with_context(|| {
            format!(
                "Cannot read download base directory '{}'; check permissions",
                self.download_base.display()
            )
        })?;
        debug!("Download base readable: {}", self.download_base.display());

        // completed_base: ensure directory and writability
        if self.completed_base.exists() && !self.completed_base.is_dir() {
            error!("Completed base exists but isn't a directory: {}", self.completed_base.display());
            bail!("Completed base exists but isn't a directory: {}", self.completed_base.display());
        }
        if !self.completed_base.exists() {
            fs::create_dir_all(&self.completed_base).with_context(|| {
                format!("Failed to create completed base directory '{}'", self.completed_base.display())
            })?;
            #[cfg(unix)]
            {
                let _ = fs::set_permissions(&self.completed_base, fs::Permissions::from_mode(0o700));
            }
            info!("Created completed base directory: {}", self.completed_base.display());
        }

        // writability probe: create & remove a small temp file
        let probe = self.completed_base.join(format!(".aria_move_probe_{}.tmp", std::process::id()));
        match fs::OpenOptions::new().create_new(true).write(true).open(&probe) {
            Ok(_) => {
                let _ = fs::remove_file(&probe);
                debug!("Completed base writable: {}", self.completed_base.display());
            }
            Err(e) => {
                error!("Cannot write to completed base '{}': {}", self.completed_base.display(), e);
                bail!(
                    "Cannot write to completed base '{}': {}. Check directory permissions.",
                    self.completed_base.display(),
                    e
                );
            }
        }

        // ensure bases are not the same (account for symlinks)
        let db_real = fs::canonicalize(&self.download_base).unwrap_or_else(|_| self.download_base.clone());
        let cb_real = fs::canonicalize(&self.completed_base).unwrap_or_else(|_| self.completed_base.clone());
        if db_real == cb_real {
            error!("Download and completed base resolve to same path: {}", db_real.display());
            bail!("Download and completed base must be different paths; both resolve to '{}'", db_real.display());
        }

        info!(
            "Config validated: download='{}' completed='{}'",
            self.download_base.display(),
            self.completed_base.display()
        );
        Ok(())
    }
}

/// Resolve the source path. If `maybe_path` is Some and exists, that wins.
/// Otherwise find the newest file under download_base modified within recent_window.
pub fn resolve_source_path(config: &Config, maybe_path: Option<&Path>) -> Result<PathBuf> {
    if let Some(p) = maybe_path {
        if p.exists() {
            return Ok(p.to_path_buf());
        }
        warn!("Provided source path does not exist: {}", p.display());
    }

    let cutoff = SystemTime::now()
        .checked_sub(config.recent_window)
        .unwrap_or(SystemTime::UNIX_EPOCH);

    let newest = WalkDir::new(&config.download_base)
        .min_depth(1)
        .max_depth(4)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|e| e.file_type().is_file())
        .filter_map(|e| e.metadata().ok().and_then(|m| m.modified().ok().map(|t| (t, e.into_path()))))
        .filter(|(modified, _)| *modified >= cutoff)
        .max_by_key(|(modified, _)| *modified);

    newest
        .map(|(_, p)| p)
        .ok_or_else(|| anyhow::anyhow!("No recently modified file found under {}", config.download_base.display()))
}

/// Top-level move entry: file or directory.
pub fn move_entry(config: &Config, src: &Path) -> Result<PathBuf> {
    ensure_not_base(&config.download_base, src)?;

    if src.is_file() {
        move_file(config, src)
    } else if src.is_dir() {
        move_dir(config, src)
    } else {
        bail!("Source path is neither file nor directory: {}", src.display())
    }
}

/// Move a single file into `completed_base`.
pub fn move_file(config: &Config, src: &Path) -> Result<PathBuf> {
    ensure_not_base(&config.download_base, src)?;
    let dest_dir = &config.completed_base;
    fs::create_dir_all(dest_dir).with_context(|| format!("Failed to create destination dir {}", dest_dir.display()))?;

    let file_name = src
        .file_name()
        .ok_or_else(|| anyhow::anyhow!("Source file missing a file name: {}", src.display()))?;
    let mut dest = dest_dir.join(file_name);

    if dest.exists() {
        dest = unique_destination(&dest);
    }

    match try_atomic_move(src, &dest) {
        Ok(()) => {
            info!(src = %src.display(), dest = %dest.display(), "Renamed file atomically");
            Ok(dest)
        }
        Err(e) => {
            warn!(error = %e, "Atomic rename failed, falling back to copy+remove");
            fs::copy(src, &dest).with_context(|| format!("Copy failed {} -> {}", src.display(), dest.display()))?;
            fs::remove_file(src).with_context(|| format!("Failed to remove original file {}", src.display()))?;
            Ok(dest)
        }
    }
}

/// Move directory contents into completed_base/<src_dir_name>.
pub fn move_dir(config: &Config, src_dir: &Path) -> Result<PathBuf> {
    ensure_not_base(&config.download_base, src_dir)?;
    let src_name = src_dir
        .file_name()
        .ok_or_else(|| anyhow::anyhow!("Source directory missing name: {}", src_dir.display()))?;
    let target = config.completed_base.join(src_name);

    // Try a fast rename first (works on same filesystem).
    if fs::rename(src_dir, &target).is_ok() {
        info!(src = %src_dir.display(), dest = %target.display(), "Renamed directory atomically");
        return Ok(target);
    }

    // Otherwise copy contents in parallel then remove source.
    WalkDir::new(src_dir)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|e| e.file_type().is_dir())
        .for_each(|d| {
            if let Ok(rel) = d.path().strip_prefix(src_dir) {
                let new_dir = target.join(rel);
                let _ = fs::create_dir_all(&new_dir);
            }
        });

    let files: Vec<_> = WalkDir::new(src_dir)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|e| e.file_type().is_file())
        .map(|e| e.into_path())
        .collect();

    files.par_iter().try_for_each(|path| -> Result<()> {
        let rel = path.strip_prefix(src_dir)?;
        let dst = target.join(rel);
        if let Some(parent) = dst.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::copy(path, &dst).with_context(|| format!("Failed copying {} -> {}", path.display(), dst.display()))?;
        Ok(())
    })?;

    fs::remove_dir_all(src_dir).with_context(|| format!("Failed to remove source directory {}", src_dir.display()))?;
    info!(src = %src_dir.display(), dest = %target.display(), "Copied directory contents and removed source");
    Ok(target)
}

/// Attempt atomic rename; caller will handle fallback behavior.
fn try_atomic_move(src: &Path, dest: &Path) -> io::Result<()> {
    fs::rename(src, dest)
}

/// Produce a unique destination when the candidate already exists.
fn unique_destination(candidate: &Path) -> PathBuf {
    if !candidate.exists() {
        return candidate.to_path_buf();
    }

    let epoch = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or_default();

    let pid = std::process::id();
    let stem = candidate.file_stem().and_then(|s| s.to_str()).unwrap_or("file");
    let ext = candidate
        .extension()
        .and_then(|s| s.to_str())
        .map(|e| format!(".{}", e))
        .unwrap_or_default();

    let new_name = format!("{}-{}-{}{}", stem, epoch, pid, ext);
    candidate.with_file_name(new_name)
}

/// Ensure we are not being asked to move the download base itself.
fn ensure_not_base(download_base: &Path, candidate: &Path) -> Result<()> {
    let base_real = fs::canonicalize(download_base).unwrap_or_else(|_| download_base.to_path_buf());
    let cand_real = fs::canonicalize(candidate).unwrap_or_else(|_| candidate.to_path_buf());

    if base_real == cand_real {
        bail!("Refusing to move the download base folder itself: {}", download_base.display())
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use assert_fs::prelude::*;
    use std::io::Write;

    #[test]
    fn unique_destination_makes_new_name_when_exists() {
        let dir = assert_fs::TempDir::new().unwrap();
        let p = dir.child("exists.txt");
        p.touch().unwrap();
        let candidate = p.path().to_path_buf();
        let new = unique_destination(&candidate);
        assert_ne!(candidate, new);
    }

    #[test]
    fn move_file_success() {
        let temp = assert_fs::TempDir::new().unwrap();
        let download = temp.child("incoming");
        let completed = temp.child("completed");
        download.create_dir_all().unwrap();
        completed.create_dir_all().unwrap();

        let source = download.child("a.txt");
        source.write_str("hello").unwrap();

        let cfg = Config::new(download.path().to_path_buf(), completed.path().to_path_buf(), RECENT_FILE_WINDOW);
        let dest = move_file(&cfg, source.path()).expect("move_file should succeed");

        assert!(dest.exists());
        assert!(!source.path().exists());
        let content = std::fs::read_to_string(&dest).unwrap();
        assert_eq!(content, "hello");
    }

    #[test]
    fn move_dir_success() {
        let temp = assert_fs::TempDir::new().unwrap();
        let download = temp.child("incoming");
        let completed = temp.child("completed");
        download.create_dir_all().unwrap();
        completed.create_dir_all().unwrap();

        let d = download.child("folder");
        d.create_dir_all().unwrap();
        let f1 = d.child("one.txt");
        f1.write_str("one").unwrap();
        let sub = d.child("sub");
        sub.create_dir_all().unwrap();
        let f2 = sub.child("two.txt");
        f2.write_str("two").unwrap();

        let cfg = Config::new(download.path().to_path_buf(), completed.path().to_path_buf(), RECENT_FILE_WINDOW);
        let src_dir = d.path();
        let dest = move_dir(&cfg, src_dir).expect("move_dir should succeed");

        assert!(dest.exists());
        assert!(dest.join("one.txt").exists());
        assert!(dest.join("sub").join("two.txt").exists());
        assert!(!src_dir.exists());
    }
}
