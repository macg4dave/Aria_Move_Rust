mod common;
#[test]
fn log_colocates_with_env_override_config() {
    common::config_environment("log_colocates_with_env_override_config", false, |root| {
        assert_eq!(
            aria_move::default_config_path().unwrap(),
            root.join("config.xml")
        );
        std::fs::write(
            root.join("config.xml"),
            "<config><disable_locks>true</disable_locks></config>",
        )
        .unwrap();
        let aria_move::config::LoadResult::Loaded(cfg, _) =
            aria_move::config::load_or_init().unwrap()
        else {
            panic!("existing config was not loaded")
        };
        assert!(cfg.disable_locks);
        assert_eq!(aria_move::default_log_path().unwrap().parent(), Some(root));
    });
}
