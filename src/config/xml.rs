//! The `config.xml` format: strict parsing and the first-run template.
use super::{Config, LogLevel};
use crate::platform;
use anyhow::{Context, Result};
use serde::Deserialize;
use std::io::Read;
use std::path::{Path, PathBuf};

/// Every element is optional; unknown elements are errors so typos surface.
#[derive(Debug, Default, Deserialize)]
#[serde(rename = "config", deny_unknown_fields)]
struct XmlConfig {
    download_base: Option<String>,
    completed_base: Option<String>,
    log_level: Option<String>,
    log_file: Option<String>,
    preserve_metadata: Option<bool>,
    preserve_permissions: Option<bool>,
    disable_locks: Option<bool>,
}

const TEMPLATE: &str = "\
<!-- Edit both placeholder paths before running. CLI values override XML. -->
<config>
  <download_base>/path/to/incoming</download_base>
  <completed_base>/path/to/completed</completed_base>
  <log_level>normal</log_level>
  <log_file></log_file>
  <preserve_metadata>false</preserve_metadata>
  <preserve_permissions>false</preserve_permissions>
  <disable_locks>false</disable_locks>
</config>
";

/// Write the template (0600 on Unix). Never replaces an existing file.
pub fn create_template_config(path: &Path) -> Result<()> {
    platform::write_new_file(path, TEMPLATE.as_bytes())
        .with_context(|| format!("write template config '{}'", path.display()))
}

/// Read and parse a config file. Symlinks, malformed XML, unknown elements
/// and invalid values are errors.
pub fn load_config(path: &Path) -> Result<Config> {
    let mut contents = String::new();
    platform::open_regular_read(path)
        .and_then(|mut file| file.read_to_string(&mut contents))
        .with_context(|| format!("read config xml '{}'", path.display()))?;
    parse(&contents).with_context(|| format!("parse config xml '{}'", path.display()))
}

fn parse(contents: &str) -> Result<Config> {
    let xml: XmlConfig = quick_xml::de::from_str(contents)?;
    let text = |value: Option<String>| value.map(|v| v.trim().to_owned()).filter(|v| !v.is_empty());
    Ok(Config {
        download_base: text(xml.download_base)
            .map(PathBuf::from)
            .unwrap_or_default(),
        completed_base: text(xml.completed_base)
            .map(PathBuf::from)
            .unwrap_or_default(),
        log_level: text(xml.log_level)
            .map(|level| level.parse::<LogLevel>())
            .transpose()
            .map_err(anyhow::Error::msg)?
            .unwrap_or_default(),
        log_file: text(xml.log_file).map(PathBuf::from),
        dry_run: false,
        preserve_metadata: xml.preserve_metadata.unwrap_or(false),
        preserve_permissions: xml.preserve_permissions.unwrap_or(false),
        disable_locks: xml.disable_locks.unwrap_or(false),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn all_fields_are_read_and_trimmed() {
        let cfg = parse(
            "<config>
               <download_base>  /in  </download_base>
               <completed_base>
                 /out
               </completed_base>
               <log_level> Info </log_level>
               <log_file> /var/log/aria_move.log </log_file>
               <preserve_metadata>true</preserve_metadata>
               <preserve_permissions>true</preserve_permissions>
               <disable_locks>true</disable_locks>
             </config>",
        )
        .unwrap();
        assert_eq!(
            cfg,
            Config {
                download_base: "/in".into(),
                completed_base: "/out".into(),
                log_level: LogLevel::Info,
                log_file: Some("/var/log/aria_move.log".into()),
                dry_run: false,
                preserve_metadata: true,
                preserve_permissions: true,
                disable_locks: true,
            }
        );
    }

    #[test]
    fn missing_and_empty_elements_mean_unset() {
        for xml in [
            "<config/>",
            "<config></config>",
            "<config><download_base></download_base><log_file>  </log_file><log_level/></config>",
        ] {
            assert_eq!(parse(xml).unwrap(), Config::default(), "{xml}");
        }
    }

    #[test]
    fn invalid_documents_are_errors() {
        for xml in [
            "<config><download_base>/in</download_base><completed_base>/out</config>",
            "<config><unknown>true</unknown></config>",
            "<config><disable_locks>maybe</disable_locks></config>",
            "<config><log_level>loud</log_level></config>",
            "",
        ] {
            assert!(parse(xml).is_err(), "{xml}");
        }
    }

    #[test]
    fn template_parses_and_is_written_exclusively() {
        let parsed = parse(TEMPLATE).unwrap();
        assert_eq!(
            parsed.download_base,
            Path::new(super::super::TEMPLATE_PLACEHOLDERS[0])
        );
        assert_eq!(
            parsed.completed_base,
            Path::new(super::super::TEMPLATE_PLACEHOLDERS[1])
        );
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("nested").join("config.xml");
        create_template_config(&path).unwrap();
        assert_eq!(load_config(&path).unwrap(), parsed);
        fs::write(&path, "<config/>").unwrap();
        assert!(create_template_config(&path).is_err());
        assert_eq!(fs::read_to_string(&path).unwrap(), "<config/>");
    }

    #[test]
    fn load_reports_the_path_on_failure() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("config.xml");
        let missing = format!("{:#}", load_config(&path).unwrap_err());
        assert!(missing.contains("read config xml"), "{missing}");
        fs::write(&path, "<config>").unwrap();
        let malformed = format!("{:#}", load_config(&path).unwrap_err());
        assert!(malformed.contains("parse config xml"), "{malformed}");
    }
}
