//! Config file handling and validation.
//!
//! Uses quick-xml + serde to parse a simple <config> XML file:
//! <config>
//!   <download_base>/path/to/incoming</download_base>
//!   <completed_base>/path/to/completed</completed_base>
//!   <log_level>info</log_level>
//! </config>

use anyhow::{bail, Context, Result};
use quick_xml::de::from_str as from_xml_str;
use serde::Deserialize;
use std::env;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::Duration;
use tracing::{debug, error, info};

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

use crate::utils::is_writable_probe;

pub const DOWNLOAD_BASE_DEFAULT: &str = "/mnt/World/incoming";
pub const COMPLETED_BASE_DEFAULT: &str = "/mnt/World/completed";
pub const RECENT_FILE_WINDOW: Duration = Duration::from_secs(5 * 60);

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
    pub fn new(download_base: impl Into<PathBuf>, completed_base: impl Into<PathBuf>, recent_window: Duration) -> Self {
        Self {
            download_base: download_base.into(),
            completed_base: completed_base.into(),
            recent_window,
            log_level: None,
        }
    }

    /// Validate existence, readability/writability and canonical paths.
    pub fn validate(&self) -> Result<()> {
        if !self.download_base.exists() {
            error!("Download base does not exist: {}", self.download_base.display());
            bail!("Download base does not exist: {}", self.download_base.display());
        }
        if !self.download_base.is_dir() {
            error!("Download base is not a directory: {}", self.download_base.display());
            bail!("Download base is not a directory: {}", self.download_base.display());
        }

        fs::read_dir(&self.download_base).with_context(|| {
            format!(
                "Cannot read download base directory '{}'; check permissions",
                self.download_base.display()
            )
        })?;
        debug!("Download base readable: {}", self.download_base.display());

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

        // writability probe
        is_writable_probe(&self.completed_base).with_context(|| {
            format!(
                "Cannot write to completed base '{}'; check permissions",
                self.completed_base.display()
            )
        })?;
        debug!("Completed base writable: {}", self.completed_base.display());

        // ensure bases not same (resolve symlinks)
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

/// Struct mirroring the XML config for deserialization.
#[derive(Debug, Deserialize)]
#[serde(rename = "config")]
struct XmlConfig {
    #[serde(rename = "download_base")]
    download_base: Option<String>,

    #[serde(rename = "completed_base")]
    completed_base: Option<String>,

    #[serde(rename = "log_level")]
    log_level: Option<String>,
}

/// Read config from XML. OS-aware default path used if ARIA_MOVE_CONFIG not set.
fn load_config_from_xml() -> Option<(PathBuf, PathBuf, Option<String>)> {
    let env_path = env::var("ARIA_MOVE_CONFIG").ok().map(PathBuf::from);

    let cfg_path = if let Some(p) = env_path.clone().or_else(default_config_path) {
        p
    } else {
        return None;
    };

    if !cfg_path.exists() {
        if env_path.is_none() {
            let _ = create_template_config(&cfg_path);
        }
        return None;
    }

    let content = fs::read_to_string(&cfg_path).ok()?;

    // Use quick-xml + serde to parse the config reliably.
    let parsed: XmlConfig = match from_xml_str(&content) {
        Ok(x) => x,
        Err(e) => {
            // parsing failure: log and fall back to defaults
            debug!("Failed to parse config.xml at {}: {}", cfg_path.display(), e);
            return None;
        }
    };

    let download_base = parsed.download_base.map(PathBuf::from);
    let completed_base = parsed.completed_base.map(PathBuf::from);
    let log_level = parsed.log_level;

    if download_base.is_none() && completed_base.is_none() && log_level.is_none() {
        return None;
    }

    Some((
        download_base.unwrap_or_else(|| PathBuf::from(DOWNLOAD_BASE_DEFAULT)),
        completed_base.unwrap_or_else(|| PathBuf::from(COMPLETED_BASE_DEFAULT)),
        log_level,
    ))
}

/// OS-appropriate default config path.
pub fn default_config_path() -> Option<PathBuf> {
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

/// Create default template config file and parent directory (best-effort permissions).
pub fn create_template_config(path: &Path) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
        #[cfg(unix)]
        {
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

/// Create default config if ARIA_MOVE_CONFIG not set; return created path so CLI can inform the user.
pub fn ensure_default_config_exists() -> Option<PathBuf> {
    if env::var("ARIA_MOVE_CONFIG").is_ok() {
        return None;
    }
    let cfg_path = default_config_path()?;
    if cfg_path.exists() {
        return None;
    }
    match create_template_config(&cfg_path) {
        Ok(()) => Some(cfg_path),
        Err(e) => {
            eprintln!("Failed to create template config at {}: {}", cfg_path.display(), e);
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::io::Write;
    use tempfile::tempdir;

    #[test]
    fn parse_valid_xml_from_env_path() {
        let dir = tempdir().unwrap();
        let cfg_path = dir.path().join("config.xml");
        let mut f = fs::File::create(&cfg_path).unwrap();
        write!(
            f,
            "<config>
                <download_base>/tmp/incoming</download_base>
                <completed_base>/tmp/completed</completed_base>
                <log_level>debug</log_level>
            </config>"
        )
        .unwrap();

        std::env::set_var("ARIA_MOVE_CONFIG", &cfg_path);
        let parsed = load_config_from_xml();
        std::env::remove_var("ARIA_MOVE_CONFIG");

        assert!(parsed.is_some());
        let (db, cb, lvl) = parsed.unwrap();
        assert_eq!(db, PathBuf::from("/tmp/incoming"));
        assert_eq!(cb, PathBuf::from("/tmp/completed"));
        assert_eq!(lvl, Some("debug".to_string()));
    }

    #[test]
    fn parse_partial_xml_returns_defaults_for_paths() {
        let dir = tempdir().unwrap();
        let cfg_path = dir.path().join("config.xml");
        let mut f = fs::File::create(&cfg_path).unwrap();
        write!(
            f,
            "<config>
                <log_level>trace</log_level>
            </config>"
        )
        .unwrap();

        std::env::set_var("ARIA_MOVE_CONFIG", &cfg_path);
        let parsed = load_config_from_xml();
        std::env::remove_var("ARIA_MOVE_CONFIG");

        assert!(parsed.is_some());
        let (db, cb, lvl) = parsed.unwrap();
        assert_eq!(db, PathBuf::from(DOWNLOAD_BASE_DEFAULT));
        assert_eq!(cb, PathBuf::from(COMPLETED_BASE_DEFAULT));
        assert_eq!(lvl, Some("trace".to_string()));
    }

    #[test]
    fn malformed_xml_yields_none() {
        let dir = tempdir().unwrap();
        let cfg_path = dir.path().join("config.xml");
        let mut f = fs::File::create(&cfg_path).unwrap();
        // intentionally malformed
        write!(f, "<config><download_base>/tmp</download_base").unwrap();

        std::env::set_var("ARIA_MOVE_CONFIG", &cfg_path);
        let parsed = load_config_from_xml();
        std::env::remove_var("ARIA_MOVE_CONFIG");

        assert!(parsed.is_none());
    }

    #[test]
    fn default_config_path_respects_home_env() {
        let dir = tempdir().unwrap();
        let home = dir.path().to_path_buf();
        std::env::set_var("HOME", &home);

        let p = default_config_path();
        assert!(p.is_some());
        // path should contain "aria_move" and "config.xml"
        let p = p.unwrap();
        assert!(p.ends_with("config.xml"));
        assert!(p.to_string().contains("aria_move"));

        std::env::remove_var("HOME");
    }

    #[test]
    fn ensure_default_config_creates_template() {
        let dir = tempdir().unwrap();
        let home = dir.path().to_path_buf();
        std::env::set_var("HOME", &home);
        std::env::remove_var("ARIA_MOVE_CONFIG");

        // Ensure file doesn't exist yet
        let p = default_config_path().expect("default path should be available");
        if p.exists() {
            let _ = fs::remove_file(&p);
        }

        let created = ensure_default_config_exists();
        assert!(created.is_some());
        let created_path = created.unwrap();
        assert!(created_path.exists());

        // content should include tag names
        let content = fs::read_to_string(&created_path).unwrap();
        assert!(content.contains("<download_base>"));
        assert!(content.contains("<completed_base>"));

        // cleanup
        let _ = fs::remove_file(&created_path);
        std::env::remove_var("HOME");
    }
}