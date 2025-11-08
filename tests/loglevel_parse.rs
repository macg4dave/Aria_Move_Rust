use aria_move::config::types::LogLevel;
use std::str::FromStr;

#[test]
fn parse_common_levels_case_insensitive() {
    assert_eq!(LogLevel::parse("quiet"), Some(LogLevel::Quiet));
    assert_eq!(LogLevel::parse("QUIET"), Some(LogLevel::Quiet));

    assert_eq!(LogLevel::parse("normal"), Some(LogLevel::Normal));
    assert_eq!(LogLevel::parse("NORMAL"), Some(LogLevel::Normal));

    assert_eq!(LogLevel::parse("info"), Some(LogLevel::Info));
    assert_eq!(LogLevel::parse("INFO"), Some(LogLevel::Info));

    assert_eq!(LogLevel::parse("verbose"), Some(LogLevel::Info));
    assert_eq!(LogLevel::parse("detailed"), Some(LogLevel::Info));

    assert_eq!(LogLevel::parse("debug"), Some(LogLevel::Debug));
    assert_eq!(LogLevel::parse("trace"), Some(LogLevel::Debug));
}

#[test]
fn display_roundtrips_with_fromstr() {
    let levels = [
        LogLevel::Quiet,
        LogLevel::Normal,
        LogLevel::Info,
        LogLevel::Debug,
    ];
    for lvl in levels {
        let s = lvl.to_string();
        let parsed = LogLevel::from_str(&s).expect("from_str should parse display string");
        assert_eq!(parsed, lvl, "roundtrip failed for {s}");
    }
}

#[test]
fn fromstr_invalid_is_err() {
    assert!(LogLevel::from_str("loud").is_err());
    assert!(LogLevel::from_str("").is_err());
}
