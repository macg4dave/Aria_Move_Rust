//! Command-line interface. aria2 invokes hooks as `<gid> <num_files> <path>`;
//! a single positional argument is treated as the path.
use crate::config::{Config, LogLevel};
use clap::{Parser, ValueHint};
use std::ffi::OsString;
use std::path::{Path, PathBuf};

#[derive(Parser, Debug, Clone)]
#[command(author, version, about = "Move completed aria2 downloads safely")]
pub struct Args {
    /// aria2 GID (informational). Alone, it is taken as the source path.
    pub task_id: Option<OsString>,

    /// Number of files reported by aria2; multi-file hooks select the download folder.
    pub num_files: Option<usize>,

    /// Source path passed by aria2.
    #[arg(value_name = "SOURCE_PATH", value_hint = ValueHint::AnyPath)]
    pub source_path_pos: Option<PathBuf>,

    /// Source path; overrides the positional SOURCE_PATH.
    #[arg(long = "source-path", short = 's', value_name = "PATH", value_hint = ValueHint::AnyPath)]
    pub source_path: Option<PathBuf>,

    /// Override the download base directory.
    #[arg(long, value_hint = ValueHint::DirPath)]
    pub download_base: Option<PathBuf>,

    /// Override the completed base directory.
    #[arg(long, value_hint = ValueHint::DirPath)]
    pub completed_base: Option<PathBuf>,

    /// Shorthand for --log-level debug.
    #[arg(short = 'd', long)]
    pub debug: bool,

    /// Log level: quiet, normal, info or debug.
    #[arg(long, value_name = "LEVEL")]
    pub log_level: Option<LogLevel>,

    /// Print the config file location and exit.
    #[arg(long)]
    pub print_config: bool,

    /// Validate configuration without moving downloads or creating directories.
    #[arg(long, conflicts_with = "init")]
    pub check_config: bool,

    /// Create a ready-to-use config using --download-base and --completed-base.
    /// Existing configuration is never overwritten.
    #[arg(long, requires_all = ["download_base", "completed_base"], conflicts_with = "dry_run")]
    pub init: bool,

    /// Show what would be moved without modifying anything.
    #[arg(long)]
    pub dry_run: bool,

    /// On cross-filesystem copies, preserve permissions, timestamps and xattrs (slower).
    #[arg(long)]
    pub preserve_metadata: bool,

    /// On cross-filesystem copies, preserve permissions only.
    #[arg(long)]
    pub preserve_permissions: bool,

    /// Disable per-download locking (for filesystems without lock support).
    #[arg(long)]
    pub disable_locks: bool,

    /// Emit structured JSON logs.
    #[arg(long)]
    pub json: bool,

    /// Path to config.xml (overrides ARIA_MOVE_CONFIG and the defaults).
    #[arg(long = "config", value_name = "PATH", value_hint = ValueHint::AnyPath)]
    pub config_path: Option<PathBuf>,
}

impl Args {
    /// `--source-path`, else the positional SOURCE_PATH, else a lone first
    /// positional (`aria_move <path>`).
    pub fn resolved_source(&self) -> Option<&Path> {
        self.source_path
            .as_deref()
            .or(self.source_path_pos.as_deref())
            .or_else(|| {
                self.num_files
                    .is_none()
                    .then(|| self.task_id.as_deref().map(Path::new))
                    .flatten()
            })
    }

    /// `--debug` wins over `--log-level`.
    pub fn effective_log_level(&self) -> Option<LogLevel> {
        if self.debug {
            Some(LogLevel::Debug)
        } else {
            self.log_level
        }
    }

    /// Apply the flags that were given; unset flags keep the config's values.
    pub fn apply_overrides(&self, cfg: &mut Config) {
        if let Some(path) = &self.download_base {
            cfg.download_base.clone_from(path);
        }
        if let Some(path) = &self.completed_base {
            cfg.completed_base.clone_from(path);
        }
        if let Some(level) = self.effective_log_level() {
            cfg.log_level = level;
        }
        cfg.dry_run |= self.dry_run;
        cfg.preserve_metadata |= self.preserve_metadata;
        cfg.preserve_permissions |= self.preserve_permissions;
        cfg.disable_locks |= self.disable_locks;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(args: &[&str]) -> Args {
        Args::try_parse_from(std::iter::once("aria_move").chain(args.iter().copied())).unwrap()
    }

    #[test]
    fn source_precedence() {
        let cases: [(&[&str], Option<&str>); 6] = [
            (&["gid", "1", "/pos"], Some("/pos")),
            (
                &["--source-path", "/flag", "gid", "1", "/pos"],
                Some("/flag"),
            ),
            (&["-s", "/flag"], Some("/flag")),
            (&["file.iso"], Some("file.iso")),
            (&["gid", "1"], None),
            (&[], None),
        ];
        for (args, expected) in cases {
            assert_eq!(
                parse(args).resolved_source(),
                expected.map(Path::new),
                "{args:?}"
            );
        }
    }

    #[test]
    fn log_level_flags() {
        assert_eq!(
            parse(&["--debug", "--log-level", "quiet"]).effective_log_level(),
            Some(LogLevel::Debug)
        );
        assert_eq!(
            parse(&["--log-level", "VERBOSE"]).effective_log_level(),
            Some(LogLevel::Info)
        );
        assert_eq!(parse(&[]).effective_log_level(), None);
        assert!(Args::try_parse_from(["aria_move", "--log-level", "loud"]).is_err());
    }

    #[test]
    fn invalid_argument_shapes_are_rejected() {
        for args in [
            &["aria_move", "gid", "not-a-number", "/p"][..],
            &["aria_move", "gid", "1", "/p", "extra"][..],
        ] {
            assert!(Args::try_parse_from(args).is_err(), "{args:?}");
        }
    }

    #[test]
    fn overrides_apply_only_when_given() {
        let mut cfg = Config::new("/in", "/out");
        cfg.disable_locks = true;
        parse(&[]).apply_overrides(&mut cfg);
        assert_eq!(cfg.download_base, PathBuf::from("/in"));
        assert!(cfg.disable_locks && !cfg.dry_run);

        parse(&[
            "--download-base",
            "/db",
            "--completed-base",
            "/cb",
            "--log-level",
            "info",
            "--dry-run",
            "--preserve-metadata",
            "--preserve-permissions",
            "--disable-locks",
        ])
        .apply_overrides(&mut cfg);
        assert_eq!(
            cfg,
            Config {
                download_base: "/db".into(),
                completed_base: "/cb".into(),
                log_level: LogLevel::Info,
                log_file: None,
                dry_run: true,
                preserve_metadata: true,
                preserve_permissions: true,
                disable_locks: true,
            }
        );
    }
}
