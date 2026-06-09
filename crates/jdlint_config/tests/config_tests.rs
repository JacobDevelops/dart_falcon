use jdlint_config::{load_config, load_or_default, find_config, JdlintConfig, RuleConfig, ConfigError};
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

static COUNTER: AtomicU64 = AtomicU64::new(0);

fn temp_file_path(name: &str) -> PathBuf {
    let id = COUNTER.fetch_add(1, Ordering::Relaxed);
    let mut path = std::env::temp_dir();
    path.push(format!("jdlint_test_{}_{}", id, name));
    path
}

fn cleanup(path: &PathBuf) {
    let _ = fs::remove_file(path);
}

#[test]
fn test_default_config() {
    let cfg = JdlintConfig::default();
    assert!(cfg.rules.is_empty());
    assert!(cfg.exclude_patterns.is_empty());
    assert!(cfg.severity_override.is_empty());
    assert_eq!(cfg.max_errors, None);
}

#[test]
fn test_load_valid_config() {
    let path = temp_file_path("valid_config.json");
    let json = r#"{
        "rules": {
            "rule1": {"enabled": true, "options": {}}
        },
        "exclude_patterns": ["*.tmp"],
        "max_errors": 100
    }"#;
    fs::write(&path, json).expect("write temp file");

    let cfg = load_config(&path).expect("load valid config");
    assert!(cfg.rules.contains_key("rule1"));
    assert_eq!(cfg.exclude_patterns, vec!["*.tmp".to_string()]);
    assert_eq!(cfg.max_errors, Some(100));

    cleanup(&path);
}

#[test]
fn test_load_invalid_json() {
    let path = temp_file_path("invalid_json.json");
    fs::write(&path, "{ invalid json }").expect("write temp file");

    let result = load_config(&path);
    assert!(result.is_err());

    cleanup(&path);
}

#[test]
fn test_load_missing_file() {
    let path = temp_file_path("nonexistent.json");
    let result = load_config(&path);
    assert!(result.is_err());
}

#[test]
fn test_load_partial_config() {
    let path = temp_file_path("partial_config.json");
    let json = r#"{"rules": {}}"#;
    fs::write(&path, json).expect("write temp file");

    let cfg = load_config(&path).expect("load partial config");
    assert!(cfg.rules.is_empty());
    assert!(cfg.exclude_patterns.is_empty());
    assert_eq!(cfg.max_errors, None);

    cleanup(&path);
}

#[test]
fn test_rule_config_default_enabled() {
    let rule = RuleConfig::default();
    assert!(rule.enabled);
    assert!(rule.options.is_empty());
}

#[test]
fn test_rule_config_disabled() {
    let path = temp_file_path("disabled_rule.json");
    let json = r#"{"rules": {"disabled_rule": {"enabled": false}}}"#;
    fs::write(&path, json).expect("write temp file");

    let cfg = load_config(&path).expect("load config");
    let rule = cfg.rules.get("disabled_rule").expect("rule exists");
    assert!(!rule.enabled);

    cleanup(&path);
}

#[test]
fn test_rule_config_with_options() {
    let path = temp_file_path("rule_with_options.json");
    let json = r#"{"rules": {"my_rule": {"enabled": true, "options": {"key": "value", "number": 42}}}}"#;
    fs::write(&path, json).expect("write temp file");

    let cfg = load_config(&path).expect("load config");
    let rule = cfg.rules.get("my_rule").expect("rule exists");
    assert!(rule.enabled);
    assert_eq!(rule.options.len(), 2);
    assert_eq!(rule.options.get("key").and_then(|v| v.as_str()), Some("value"));
    assert_eq!(rule.options.get("number").and_then(|v| v.as_i64()), Some(42));

    cleanup(&path);
}

#[test]
fn test_find_config_in_cwd() {
    let dir = std::env::temp_dir();
    let config_path = dir.join("jdlint.json");
    fs::write(&config_path, "{}").expect("write config");

    let found = find_config(&dir);
    assert_eq!(found, Some(config_path.clone()));

    fs::remove_file(&config_path).expect("cleanup");
}

#[test]
fn test_find_config_none_when_no_local_or_git_config() {
    // Create a temp dir with no jdlint.json and no parent .git
    let id = COUNTER.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("jdlint_test_no_config_{}", id));
    fs::create_dir_all(&dir).expect("create temp dir");

    // Verify there's no local or git-root config (the only ambiguity is $HOME/.jdlint.json)
    assert!(!dir.join("jdlint.json").exists());

    let found = find_config(&dir);

    // If $HOME/.jdlint.json doesn't exist we expect None; if it does the function
    // correctly returns it (valid behaviour per the discovery priority spec).
    let home_config_exists = std::env::var("HOME")
        .map(|h| std::path::Path::new(&h).join(".jdlint.json").exists())
        .unwrap_or(false);

    if !home_config_exists {
        assert!(
            found.is_none(),
            "expected no config to be found but got: {:?}",
            found
        );
    }

    let _ = fs::remove_dir(&dir);
}

#[test]
fn test_config_error_display() {
    let err = ConfigError("test error message".to_string());
    let displayed = err.to_string();
    assert!(displayed.contains("test error message"));
}

#[test]
fn test_load_or_default_missing() {
    let id = COUNTER.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("jdlint_test_load_or_default_{}", id));
    fs::create_dir_all(&dir).expect("create temp dir");

    let cfg = load_or_default(&dir);
    assert!(cfg.rules.is_empty());
    assert_eq!(cfg.max_errors, None);

    let _ = fs::remove_dir(&dir);
}

#[test]
fn test_load_or_default_with_file() {
    let id = COUNTER.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("jdlint_test_load_or_default_file_{}", id));
    fs::create_dir_all(&dir).expect("create temp dir");

    let config_path = dir.join("jdlint.json");
    let json = r#"{"rules": {"test": {"enabled": true}}, "max_errors": 50}"#;
    fs::write(&config_path, json).expect("write config");

    let cfg = load_or_default(&dir);
    assert!(cfg.rules.contains_key("test"));
    assert_eq!(cfg.max_errors, Some(50));

    fs::remove_file(&config_path).expect("cleanup config");
    let _ = fs::remove_dir(&dir);
}
