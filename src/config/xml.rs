//! One fallible XML mapping and one exclusive template writer.
use super::paths::{default_config_path, reject_symlinks};
use super::types::{Config, LogLevel};
use super::{COMPLETED_BASE_DEFAULT, DOWNLOAD_BASE_DEFAULT};
use anyhow::{Context, Result};
use quick_xml::de::from_str as from_xml_str;
use serde::Deserialize;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Deserialize)]
#[serde(rename = "config")]
#[serde(deny_unknown_fields)]
struct XmlConfig {
    #[serde(rename = "download_base")]
    download_base: Option<String>,
    #[serde(rename = "completed_base")]
    completed_base: Option<String>,
    #[serde(rename = "log_level")]
    log_level: Option<String>,
    #[serde(rename = "log_file")]
    log_file: Option<String>,
    #[serde(rename = "preserve_metadata")]
    preserve_metadata: Option<bool>,
    #[serde(rename = "preserve_permissions")]
    preserve_permissions: Option<bool>,
    #[serde(rename = "disable_locks")]
    disable_locks: Option<bool>,
}

// Legacy tuple API retained as a compatibility adapter; runtime uses Config.
type LoadedConfig = (
    PathBuf,
    PathBuf,
    Option<LogLevel>,
    Option<PathBuf>,
    bool,
    bool,
    bool,
);

/// Legacy optional loader. Errors are diagnosed; callers needing failure handling
/// should use load_config_from_xml_path instead.
pub fn load_config_from_xml() -> Option<LoadedConfig> {
    let path = default_config_path().ok()?;
    match load_config_from_xml_path(&path) {
        Ok(cfg) => Some((
            cfg.download_base,
            cfg.completed_base,
            Some(cfg.log_level),
            cfg.log_file,
            cfg.preserve_metadata,
            cfg.preserve_permissions,
            cfg.disable_locks,
        )),
        Err(e) => {
            tracing::warn!(error = %e, "Unable to load config");
            None
        }
    }
}

/// Write a complete template without replacing an existing file.
pub fn create_template_config(path: &Path) -> Result<()> {
    reject_symlinks(path)?;
    let content = "<!-- Edit both placeholder paths before running. CLI values override XML. -->\n<config>\n  <download_base>/path/to/incoming</download_base>\n  <completed_base>/path/to/completed</completed_base>\n  <log_level>normal</log_level>\n  <log_file></log_file>\n  <preserve_metadata>false</preserve_metadata>\n  <preserve_permissions>false</preserve_permissions>\n  <disable_locks>false</disable_locks>\n</config>\n";
    crate::platform::write_config_secure_new_0600(path, content.as_bytes())
}

/// Best-effort compatibility entry point for default template creation.
pub fn ensure_default_config_exists() -> Option<PathBuf> {
    if std::env::var_os("ARIA_MOVE_CONFIG").is_some() {
        return None;
    }
    let path = default_config_path().ok()?;
    match create_template_config(&path) {
        Ok(()) => Some(path),
        Err(e) => {
            tracing::debug!(error = %e, "Default config not created");
            None
        }
    }
}

fn xml_to_config(parsed: XmlConfig) -> Result<Config> {
    let default_cfg = Config::default();
    let download_base = parsed
        .download_base
        .as_deref()
        .map(|s| PathBuf::from(s.trim()))
        .unwrap_or_else(|| PathBuf::from(DOWNLOAD_BASE_DEFAULT));
    let completed_base = parsed
        .completed_base
        .as_deref()
        .map(|s| PathBuf::from(s.trim()))
        .unwrap_or_else(|| PathBuf::from(COMPLETED_BASE_DEFAULT));
    let log_file = match parsed.log_file.as_deref().map(str::trim) {
        Some(s) if !s.is_empty() => Some(PathBuf::from(s)),
        _ => default_cfg.log_file.clone(),
    };
    let log_level = parsed
        .log_level
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::parse)
        .transpose()
        .map_err(anyhow::Error::msg)?
        .unwrap_or(default_cfg.log_level);
    let preserve_metadata = parsed.preserve_metadata.unwrap_or(false);
    let preserve_permissions = if preserve_metadata {
        false
    } else {
        parsed.preserve_permissions.unwrap_or(false)
    };
    let disable_locks = parsed.disable_locks.unwrap_or(false);
    Ok(Config {
        download_base,
        completed_base,
        log_level,
        log_file,
        dry_run: false,
        preserve_metadata,
        preserve_permissions,
        disable_locks,
    })
}

/// Parse an explicit config path. Unknown fields and malformed XML return errors.
pub fn load_config_from_xml_path(path: &Path) -> Result<Config> {
    load_for_app(path).map(|loaded| loaded.config)
}

/// Settings and explicit log selection from one XML read.
#[derive(Debug)]
pub struct LoadedSettings {
    pub config: Config,
    pub explicit_log: Option<PathBuf>,
}

/// Load settings together with the explicit log path for dry-run policy.
pub fn load_for_app(path: &Path) -> Result<LoadedSettings> {
    reject_symlinks(path)?;
    let mut file = crate::platform::open_regular_read(path)?;
    let mut contents = String::new();
    std::io::Read::read_to_string(&mut file, &mut contents)
        .with_context(|| format!("read config xml '{}'", path.display()))?;
    let parsed: XmlConfig = from_xml_str(&contents)
        .with_context(|| format!("parse config xml '{}'", path.display()))?;
    let explicit_log = parsed
        .log_file
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(PathBuf::from);
    Ok(LoadedSettings {
        config: xml_to_config(parsed)?,
        explicit_log,
    })
}

/// Load an environment override, including a directory override's config.xml.
pub fn load_config_from_xml_env() -> Result<Option<Config>> {
    if std::env::var_os("ARIA_MOVE_CONFIG").is_some() {
        return load_config_from_xml_path(&default_config_path()?).map(Some);
    }
    Ok(None)
}

/// Load the selected default path, returning None only when it is missing.
pub fn load_config_from_default_xml() -> Result<Option<Config>> {
    let path = default_config_path()?;
    match fs::symlink_metadata(&path) {
        Ok(_) => load_config_from_xml_path(&path).map(Some),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e.into()),
    }
}

/// Outcome of loading an existing config or creating a template for editing.
#[derive(Debug)]
pub enum LoadResult {
    Loaded(Config, PathBuf),
    CreatedTemplate(PathBuf),
}

/// Load the selected configuration, creating a template only when it is missing.
pub fn load_or_init() -> Result<LoadResult> {
    let path = default_config_path()?;
    match fs::symlink_metadata(&path) {
        Ok(_) => Ok(LoadResult::Loaded(load_config_from_xml_path(&path)?, path)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            create_template_config(&path)?;
            Ok(LoadResult::CreatedTemplate(path))
        }
        Err(e) => Err(e).with_context(|| format!("stat config {}", path.display())),
    }
}

/// Ensure a default configuration exists and return its path.
pub fn ensure_selected_config_exists() -> Result<PathBuf> {
    match load_or_init()? {
        LoadResult::Loaded(_, path) | LoadResult::CreatedTemplate(path) => Ok(path),
    }
}
