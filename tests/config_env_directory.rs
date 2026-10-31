mod common;
#[test]
fn env_override_directory_appends_config_xml_and_colocates_log() {
    common::config_environment(
        "env_override_directory_appends_config_xml_and_colocates_log",
        true,
        |root| {
            assert_eq!(
                aria_move::default_config_path().unwrap(),
                root.join("config.xml")
            );
            assert_eq!(aria_move::default_log_path().unwrap().parent(), Some(root));
        },
    );
}
