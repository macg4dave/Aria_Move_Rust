//! Explicit source resolution; no automatic download selection.
use crate::{AriaMoveError, Config};
use anyhow::Result;
use std::path::{Path, PathBuf};

pub fn resolve_source_path(config: &Config, maybe_path: Option<&Path>) -> Result<PathBuf> {
    let path = maybe_path.ok_or_else(|| AriaMoveError::NoneFound(config.download_base.clone()))?;
    let selected = match std::fs::symlink_metadata(path) {
        Ok(_) => path.to_path_buf(),
        Err(e)
            if e.kind() == std::io::ErrorKind::NotFound
                && !path.has_root()
                && path.components().count() == 1
                && path.file_name().is_some() =>
        {
            config.download_base.join(path)
        }
        Err(e) => {
            return Err(crate::errors::io_context(path, e));
        }
    };
    let meta = std::fs::symlink_metadata(&selected)
        .map_err(|e| crate::errors::io_context(&selected, e))?;
    crate::config::paths::reject_symlinks(&selected)?;
    if !meta.is_file() && !meta.is_dir() {
        return Err(AriaMoveError::ProvidedNotFile(selected).into());
    }
    Ok(selected)
}

/// Resolve a CLI download path and promote nested files to their download's
/// immediate child directory. Library resolve_source_path preserves explicit files.
#[doc(hidden)]
pub fn resolve_download_source(config: &Config, path: Option<&Path>) -> Result<PathBuf> {
    // Resolve the exact OS path (including a bare quoted name under the base)
    // before trying historical shell-quote tolerance. Never rewrite a valid name.
    let selected = match resolve_source_path(config, path) {
        Ok(selected) => selected,
        Err(error)
            if error.downcast_ref::<std::io::Error>().is_some_and(|cause| {
                matches!(
                    cause.kind(),
                    std::io::ErrorKind::NotFound | std::io::ErrorKind::InvalidFilename
                )
            }) =>
        {
            let decoded = path.and_then(decode_legacy_quotes);
            match decoded {
                Some(decoded) => resolve_source_path(config, Some(&decoded))?,
                None => return Err(error),
            }
        }
        Err(error) => return Err(error),
    };
    if !selected.is_file() {
        return Ok(selected);
    }
    let absolute = dunce::canonicalize(&selected)?;
    let base = dunce::canonicalize(&config.download_base)?;
    if let Ok(relative) = absolute.strip_prefix(&base)
        && relative.components().count() > 1
        && let Some(first) = relative.components().next()
    {
        return Ok(base.join(first));
    }
    Ok(selected)
}

fn decode_legacy_quotes(path: &Path) -> Option<PathBuf> {
    let text = path.to_str()?;
    for quote in ['\'', '"'] {
        if let Some(rest) = text.strip_prefix(quote) {
            let rest = rest.strip_suffix('/').unwrap_or(rest);
            if let Some(inner) = rest.strip_suffix(quote) {
                let inner = if inner.len() > 3 {
                    inner.strip_suffix('\\').unwrap_or(inner)
                } else {
                    inner
                };
                return Some(PathBuf::from(inner));
            }
        }
    }
    None
}
