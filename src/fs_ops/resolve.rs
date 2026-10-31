//! Turn the path aria2 or a user passes into the entry to move. Nothing is
//! ever selected automatically.
use crate::{AriaMoveError, Config, errors, platform};
use anyhow::{Result, bail};
use std::io;
use std::path::{Component, Path, PathBuf};

/// Resolve the source to move:
/// 1. the path as given; a bare name that does not exist relative to the
///    current directory is looked up in the download base;
/// 2. if not found, a path wrapped in literal quotes (as some shells and
///    wrappers pass it) is retried without them — an existing literal name
///    always wins;
/// 3. a file nested inside a download folder (aria2 passes the first file of a
///    multi-file download) is promoted to that top-level folder.
pub fn resolve_source_path(config: &Config, path: Option<&Path>) -> Result<PathBuf> {
    let path = path.ok_or_else(|| AriaMoveError::NoneFound(config.download_base.clone()))?;
    let selected = match locate(config, path) {
        Err(error) if is_missing(&error) => match decode_legacy_quotes(path) {
            Some(decoded) => locate(config, &decoded)?,
            None => return Err(error),
        },
        result => result?,
    };
    Ok(promote_to_download_root(config, selected))
}

fn locate(config: &Config, path: &Path) -> Result<PathBuf> {
    // Without trailing separators a final symlink is inspected, not followed.
    let path: PathBuf = path.components().collect();
    let selected = match std::fs::symlink_metadata(&path) {
        Ok(_) => path,
        Err(e) if e.kind() == io::ErrorKind::NotFound && is_bare_name(&path) => {
            config.download_base.join(&path)
        }
        Err(e) => return Err(errors::source_error(&path, e)),
    };
    let meta =
        std::fs::symlink_metadata(&selected).map_err(|e| errors::source_error(&selected, e))?;
    if platform::is_link(&meta) {
        bail!("Refusing to move symlink: {}", selected.display());
    }
    if !meta.is_file() && !meta.is_dir() {
        return Err(AriaMoveError::ProvidedNotFile(selected).into());
    }
    // Check before canonicalization can erase a linked ancestor during promotion.
    platform::reject_symlinks(&selected)?;
    Ok(selected)
}

fn is_bare_name(path: &Path) -> bool {
    let mut components = path.components();
    matches!(
        (components.next(), components.next()),
        (Some(Component::Normal(_)), None)
    )
}

fn is_missing(error: &anyhow::Error) -> bool {
    error
        .chain()
        .filter_map(|cause| cause.downcast_ref::<io::Error>())
        .any(|e| {
            matches!(
                e.kind(),
                io::ErrorKind::NotFound | io::ErrorKind::InvalidFilename
            )
        })
}

/// `'path'`, `"path"`, `'path'/` and PowerShell's `'path\'` lose their quotes.
fn decode_legacy_quotes(path: &Path) -> Option<PathBuf> {
    let text = path.to_str()?;
    ['\'', '"'].into_iter().find_map(|quote| {
        let rest = text.strip_prefix(quote)?;
        let rest = rest.strip_suffix('/').unwrap_or(rest);
        let inner = rest.strip_suffix(quote)?;
        // Keep a lone root such as `C:\` intact.
        let inner = if inner.len() > 3 {
            inner.strip_suffix('\\').unwrap_or(inner)
        } else {
            inner
        };
        Some(PathBuf::from(inner))
    })
}

fn promote_to_download_root(config: &Config, selected: PathBuf) -> PathBuf {
    if !selected.is_file() {
        return selected;
    }
    let (Ok(absolute), Ok(base)) = (
        dunce::canonicalize(&selected),
        dunce::canonicalize(&config.download_base),
    ) else {
        return selected;
    };
    match absolute.strip_prefix(&base).map(Path::components) {
        Ok(mut relative) if relative.clone().count() > 1 => match relative.next() {
            Some(first) => base.join(first),
            None => selected,
        },
        _ => selected,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn fixture() -> (tempfile::TempDir, Config) {
        let temp = tempfile::tempdir().unwrap();
        let base = dunce::canonicalize(temp.path()).unwrap().join("incoming");
        fs::create_dir(&base).unwrap();
        let config = Config::new(base, temp.path().join("completed"));
        (temp, config)
    }

    #[test]
    fn missing_path_argument_is_none_found() {
        let (_temp, cfg) = fixture();
        let error = resolve_source_path(&cfg, None).unwrap_err();
        assert_eq!(errors::error_code(&error), "none_found");
    }

    #[test]
    fn explicit_and_bare_paths_resolve() {
        let (_temp, cfg) = fixture();
        let base = &cfg.download_base;
        fs::write(base.join("file.iso"), b"x").unwrap();
        fs::create_dir(base.join("New folder")).unwrap();
        let cases = [
            (base.join("file.iso"), base.join("file.iso")),
            (PathBuf::from("file.iso"), base.join("file.iso")),
            (PathBuf::from("New folder"), base.join("New folder")),
            (PathBuf::from("New folder/"), base.join("New folder")),
            (base.join("New folder").join(""), base.join("New folder")),
        ];
        for (input, expected) in cases {
            assert_eq!(
                resolve_source_path(&cfg, Some(&input)).unwrap(),
                expected,
                "{input:?}"
            );
        }
    }

    #[test]
    fn missing_sources_are_typed_errors() {
        let (_temp, cfg) = fixture();
        for input in ["absent.bin", "sub/absent.bin"] {
            let error = resolve_source_path(&cfg, Some(Path::new(input))).unwrap_err();
            assert_eq!(errors::error_code(&error), "source_not_found", "{input}");
        }
    }

    #[test]
    fn nested_files_are_promoted_to_their_download_folder() {
        let (_temp, cfg) = fixture();
        let base = &cfg.download_base;
        fs::create_dir_all(base.join("album").join("disc 2")).unwrap();
        fs::write(base.join("album").join("disc 2").join("track.flac"), b"x").unwrap();
        fs::write(base.join("single.bin"), b"x").unwrap();
        let nested = base.join("album").join("disc 2").join("track.flac");
        assert_eq!(
            resolve_source_path(&cfg, Some(&nested)).unwrap(),
            base.join("album")
        );
        let top = base.join("single.bin");
        assert_eq!(resolve_source_path(&cfg, Some(&top)).unwrap(), top);
        let directory = base.join("album").join("disc 2");
        assert_eq!(
            resolve_source_path(&cfg, Some(&directory)).unwrap(),
            directory
        );
    }

    #[test]
    fn files_outside_the_download_base_are_not_promoted() {
        let (temp, cfg) = fixture();
        let outside = dunce::canonicalize(temp.path()).unwrap().join("elsewhere");
        fs::create_dir_all(outside.join("dir")).unwrap();
        fs::write(outside.join("dir").join("f"), b"x").unwrap();
        let file = outside.join("dir").join("f");
        assert_eq!(resolve_source_path(&cfg, Some(&file)).unwrap(), file);
    }

    #[test]
    fn legacy_quote_decoding() {
        let cases = [
            ("'a b'", Some("a b")),
            ("\"a b\"", Some("a b")),
            ("'dir'/", Some("dir")),
            ("'C:\\dir\\'", Some("C:\\dir")),
            ("'C:\\'", Some("C:\\")),
            ("'unterminated", None),
            ("plain", None),
            ("'mixed\"", None),
        ];
        for (input, expected) in cases {
            assert_eq!(
                decode_legacy_quotes(Path::new(input)),
                expected.map(PathBuf::from),
                "{input}"
            );
        }
    }

    #[test]
    fn quotes_are_only_removed_when_the_literal_name_is_missing() {
        let (_temp, cfg) = fixture();
        let base = &cfg.download_base;
        fs::write(base.join("name"), b"unquoted").unwrap();
        assert_eq!(
            resolve_source_path(&cfg, Some(Path::new("'name'"))).unwrap(),
            base.join("name")
        );
        #[cfg(unix)]
        {
            // Quotes are legal in Unix names; the literal name must win.
            fs::write(base.join("'name'"), b"quoted").unwrap();
            assert_eq!(
                resolve_source_path(&cfg, Some(Path::new("'name'"))).unwrap(),
                base.join("'name'")
            );
        }
    }

    #[cfg(unix)]
    #[test]
    fn linked_ancestors_are_rejected_before_promotion() {
        let (_temp, cfg) = fixture();
        fs::create_dir_all(cfg.download_base.join("album/disc")).unwrap();
        fs::write(cfg.download_base.join("album/disc/track"), b"keep").unwrap();
        std::os::unix::fs::symlink(
            cfg.download_base.join("album/disc"),
            cfg.download_base.join("alias"),
        )
        .unwrap();
        let error =
            resolve_source_path(&cfg, Some(&cfg.download_base.join("alias/track"))).unwrap_err();
        assert!(error.to_string().contains("symlink"), "{error:#}");
    }

    #[cfg(unix)]
    #[test]
    fn symlinks_and_special_files_are_refused() {
        let (temp, cfg) = fixture();
        let base = &cfg.download_base;
        let target = temp.path().join("target");
        fs::create_dir(&target).unwrap();
        std::os::unix::fs::symlink(&target, base.join("link")).unwrap();
        for input in ["link", "link/"] {
            let error = resolve_source_path(&cfg, Some(Path::new(input))).unwrap_err();
            assert!(error.to_string().contains("symlink"), "{input}: {error}");
        }
        let fifo = base.join("pipe");
        assert!(
            std::process::Command::new("mkfifo")
                .arg(&fifo)
                .status()
                .unwrap()
                .success()
        );
        let error = resolve_source_path(&cfg, Some(&fifo)).unwrap_err();
        assert_eq!(errors::error_code(&error), "provided_not_file");
    }
}
