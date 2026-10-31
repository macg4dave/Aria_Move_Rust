use std::path::Path;
pub fn write_cfg(path: &Path, download: &Path, completed: &Path) {
    let download =
        quick_xml::escape::escape(download.to_str().expect("test fixture path is Unicode"))
            .into_owned();
    let completed =
        quick_xml::escape::escape(completed.to_str().expect("test fixture path is Unicode"))
            .into_owned();
    let xml = format!(
        "<config><download_base>{download}</download_base><completed_base>{completed}</completed_base><log_level>quiet</log_level><log_file></log_file><preserve_metadata>false</preserve_metadata><preserve_permissions>false</preserve_permissions></config>"
    );
    std::fs::write(path, xml).unwrap();
}
