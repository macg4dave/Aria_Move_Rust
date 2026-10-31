//! Runtime configuration: types, XML loading, default locations and validation.
pub mod paths;
pub mod xml;

use crate::platform;
use anyhow::{Result, bail};
use std::fmt;
use std::path::{Path, PathBuf};
use std::str::FromStr;

pub use paths::{default_config_path, default_log_path, resolve_config_path};
pub use xml::{create_template_config, load_config};

/// Verbosity exposed to users and config files.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LogLevel {
    /// Errors only.
    Quiet,
    /// Informational output.
    #[default]
    Normal,
    /// Verbose output.
    Info,
    /// Everything, including traces.
    Debug,
}

impl LogLevel {
    /// The tracing threshold for this level.
    pub fn filter(self) -> tracing::level_filters::LevelFilter {
        use tracing::level_filters::LevelFilter;
        match self {
            Self::Quiet => LevelFilter::ERROR,
            Self::Normal => LevelFilter::INFO,
            Self::Info => LevelFilter::DEBUG,
            Self::Debug => LevelFilter::TRACE,
        }
    }
}

impl FromStr for LogLevel {
    type Err = String;

    /// Case-insensitive, with common aliases.
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_ascii_lowercase().as_str() {
            "quiet" | "error" | "none" => Ok(Self::Quiet),
            "normal" => Ok(Self::Normal),
            "info" | "verbose" | "detailed" => Ok(Self::Info),
            "debug" | "trace" => Ok(Self::Debug),
            _ => Err(format!(
                "invalid log level '{s}' (expected quiet, normal, info or debug)"
            )),
        }
    }
}

impl fmt::Display for LogLevel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Quiet => "quiet",
            Self::Normal => "normal",
            Self::Info => "info",
            Self::Debug => "debug",
        })
    }
}

/// Settings for one move. Bases are empty until configured.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Config {
    /// Where downloads are written while in progress.
    pub download_base: PathBuf,
    /// Where completed downloads are moved.
    pub completed_base: PathBuf,
    pub log_level: LogLevel,
    /// Explicit log file. When unset the binary logs beside the config file,
    /// except during a dry run, which never creates a default log.
    pub log_file: Option<PathBuf>,
    /// Report the destination without modifying anything.
    pub dry_run: bool,
    /// On copies, preserve timestamps, permissions and (with the `xattrs`
    /// feature) extended attributes.
    pub preserve_metadata: bool,
    /// On copies, preserve permissions only. Implied by `preserve_metadata`.
    pub preserve_permissions: bool,
    /// Skip per-download locks (for filesystems without lock support).
    pub disable_locks: bool,
}

impl Config {
    pub fn new(download_base: impl Into<PathBuf>, completed_base: impl Into<PathBuf>) -> Self {
        Self {
            download_base: download_base.into(),
            completed_base: completed_base.into(),
            ..Self::default()
        }
    }
}

/// Placeholder bases written into the first-run template.
pub(crate) const TEMPLATE_PLACEHOLDERS: [&str; 2] = ["/path/to/incoming", "/path/to/completed"];

/// Validate the bases and replace them with normalized absolute paths.
/// Missing bases are created, except in a dry run, which never writes.
pub fn validate_and_normalize(cfg: &mut Config) -> Result<()> {
    for (name, path) in [
        ("download_base", &cfg.download_base),
        ("completed_base", &cfg.completed_base),
    ] {
        if path.as_os_str().is_empty() {
            bail!(
                "{name} is not set; edit config.xml or pass --{}",
                name.replace('_', "-")
            );
        }
        if TEMPLATE_PLACEHOLDERS.iter().any(|p| path == Path::new(p)) {
            bail!(
                "{name} is still the template placeholder '{}'; edit config.xml",
                path.display()
            );
        }
        platform::reject_symlinks(path)?;
        if path.exists() && !path.is_dir() {
            bail!("{name} '{}' exists but is not a directory", path.display());
        }
    }
    let download = platform::normalize_missing(&cfg.download_base)?;
    let completed = platform::normalize_missing(&cfg.completed_base)?;
    if download == completed {
        bail!(
            "download_base and completed_base resolve to the same path: '{}'",
            download.display()
        );
    }
    if download.starts_with(&completed) || completed.starts_with(&download) {
        bail!(
            "download_base '{}' and completed_base '{}' must not be nested",
            download.display(),
            completed.display()
        );
    }
    if !cfg.dry_run {
        platform::create_directory(&download)?;
        platform::create_directory(&completed)?;
    }
    cfg.download_base = download;
    cfg.completed_base = completed;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn log_level_parsing_aliases_and_round_trip() {
        let cases = [
            ("quiet", LogLevel::Quiet),
            ("ERROR", LogLevel::Quiet),
            ("none", LogLevel::Quiet),
            ("Normal", LogLevel::Normal),
            ("info", LogLevel::Info),
            ("verbose", LogLevel::Info),
            ("detailed", LogLevel::Info),
            ("DEBUG", LogLevel::Debug),
            ("trace", LogLevel::Debug),
        ];
        for (text, level) in cases {
            assert_eq!(text.parse::<LogLevel>(), Ok(level), "{text}");
            assert_eq!(level.to_string().parse::<LogLevel>(), Ok(level));
        }
        for invalid in ["", "loud", " info"] {
            assert!(invalid.parse::<LogLevel>().is_err(), "{invalid:?}");
        }
    }

    fn root() -> (tempfile::TempDir, PathBuf) {
        let temp = tempfile::tempdir().unwrap();
        let root = dunce::canonicalize(temp.path()).unwrap();
        (temp, root)
    }

    #[test]
    fn validation_creates_missing_bases_and_normalizes() {
        let (_temp, root) = root();
        let mut cfg = Config::new(
            root.join("in").join("."),
            root.join("x").join("..").join("out"),
        );
        validate_and_normalize(&mut cfg).unwrap();
        assert_eq!(cfg.download_base, root.join("in"));
        assert_eq!(cfg.completed_base, root.join("out"));
        assert!(cfg.download_base.is_dir() && cfg.completed_base.is_dir());
    }

    #[test]
    fn dry_run_validation_never_creates_directories() {
        let (_temp, root) = root();
        let mut cfg = Config::new(root.join("in"), root.join("deep").join("out"));
        cfg.dry_run = true;
        validate_and_normalize(&mut cfg).unwrap();
        assert_eq!(fs::read_dir(&root).unwrap().count(), 0);
    }

    #[test]
    fn invalid_base_combinations_are_rejected_without_writes() {
        let (_temp, root) = root();
        let file = root.join("file");
        fs::write(&file, b"x").unwrap();
        let cases: [(PathBuf, PathBuf, &str); 7] = [
            (PathBuf::new(), root.join("out"), "download_base is not set"),
            (root.join("in"), PathBuf::new(), "completed_base is not set"),
            (
                "/path/to/incoming".into(),
                root.join("out"),
                "template placeholder",
            ),
            (file.clone(), root.join("out"), "not a directory"),
            (root.join("same"), root.join("same"), "same path"),
            (
                root.join("a"),
                root.join("a").join("b"),
                "must not be nested",
            ),
            (
                root.join("a").join("b"),
                root.join("a"),
                "must not be nested",
            ),
        ];
        for (download, completed, message) in cases {
            let mut cfg = Config::new(download, completed);
            let error = validate_and_normalize(&mut cfg).unwrap_err().to_string();
            assert!(error.contains(message), "expected {message:?} in {error:?}");
        }
        assert_eq!(
            fs::read_dir(&root).unwrap().count(),
            1,
            "only the fixture file"
        );
    }

    #[cfg(unix)]
    #[test]
    fn bases_under_symlinks_are_rejected() {
        let (_temp, root) = root();
        fs::create_dir(root.join("real")).unwrap();
        std::os::unix::fs::symlink(root.join("real"), root.join("link")).unwrap();
        let mut cfg = Config::new(root.join("link").join("in"), root.join("out"));
        assert!(validate_and_normalize(&mut cfg).is_err());
    }
}
