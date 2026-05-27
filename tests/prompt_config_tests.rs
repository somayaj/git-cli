use git_cli::config::{PromptConfig, PromptExample};

#[test]
fn default_has_no_preamble() {
    let cfg = PromptConfig::default();
    assert!(cfg.preamble.is_none());
}

#[test]
fn default_has_no_examples() {
    let cfg = PromptConfig::default();
    assert!(cfg.examples.is_empty());
}

#[test]
fn parse_preamble_override() {
    let toml_str = r#"
preamble = "You are a DevOps bot."
"#;
    let cfg: PromptConfig = toml::from_str(toml_str).unwrap();
    assert_eq!(cfg.preamble.unwrap(), "You are a DevOps bot.");
    assert!(cfg.examples.is_empty());
}

#[test]
fn parse_single_example() {
    let toml_str = r#"
[[examples]]
task = "tag v1.0"
commands = "git tag v1.0"
"#;
    let cfg: PromptConfig = toml::from_str(toml_str).unwrap();
    assert!(cfg.preamble.is_none());
    assert_eq!(cfg.examples.len(), 1);
    assert_eq!(cfg.examples[0].task, "tag v1.0");
    assert_eq!(cfg.examples[0].commands, "git tag v1.0");
}

#[test]
fn parse_multiple_examples() {
    let toml_str = r#"
[[examples]]
task = "tag v1.0"
commands = "git tag v1.0"

[[examples]]
task = "show diff"
commands = "git diff main..develop"
"#;
    let cfg: PromptConfig = toml::from_str(toml_str).unwrap();
    assert_eq!(cfg.examples.len(), 2);
    assert_eq!(cfg.examples[1].task, "show diff");
}

#[test]
fn parse_preamble_and_examples_together() {
    let toml_str = r#"
preamble = "Custom preamble."

[[examples]]
task = "stash changes"
commands = "git stash"
"#;
    let cfg: PromptConfig = toml::from_str(toml_str).unwrap();
    assert_eq!(cfg.preamble.unwrap(), "Custom preamble.");
    assert_eq!(cfg.examples.len(), 1);
}

#[test]
fn parse_multiline_commands() {
    let toml_str = r#"
[[examples]]
task = "create and push tag"
commands = """
# Create annotated tag
git tag -a v1.0 -m "Release"
# Push to remote
git push origin v1.0"""
"#;
    let cfg: PromptConfig = toml::from_str(toml_str).unwrap();
    assert!(cfg.examples[0].commands.contains("git tag"));
    assert!(cfg.examples[0].commands.contains("git push"));
}

#[test]
fn malformed_toml_falls_back_to_default() {
    let bad_toml = r#"
this is not valid toml {{{
"#;
    let cfg: Result<PromptConfig, _> = toml::from_str(bad_toml);
    assert!(cfg.is_err());
    // PromptConfig::load() would call unwrap_or_default() on this error
    let fallback = cfg.unwrap_or_default();
    assert!(fallback.preamble.is_none());
    assert!(fallback.examples.is_empty());
}

#[test]
fn empty_toml_gives_defaults() {
    let cfg: PromptConfig = toml::from_str("").unwrap();
    assert!(cfg.preamble.is_none());
    assert!(cfg.examples.is_empty());
}

#[test]
fn extra_fields_are_ignored() {
    let toml_str = r#"
preamble = "Hello"
unknown_field = "should be ignored"
"#;
    // serde's default behavior with deny_unknown_fields off
    let cfg: PromptConfig = toml::from_str(toml_str).unwrap();
    assert_eq!(cfg.preamble.unwrap(), "Hello");
}

#[test]
fn example_round_trip_serialize() {
    let ex = PromptExample {
        task: "show log".to_string(),
        commands: "git log --oneline".to_string(),
    };
    let serialized = toml::to_string(&ex).unwrap();
    assert!(serialized.contains("show log"));
    assert!(serialized.contains("git log --oneline"));
}

#[test]
fn config_path_is_under_dot_config() {
    if let Some(path) = PromptConfig::config_path() {
        let path_str = path.to_string_lossy();
        assert!(path_str.contains(".config/git-cli/prompt.toml"));
    }
}

#[test]
fn config_dir_is_parent_of_config_path() {
    if let (Some(dir), Some(path)) = (PromptConfig::config_dir(), PromptConfig::config_path()) {
        assert_eq!(path.parent().unwrap(), dir);
    }
}
