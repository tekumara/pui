use anyhow::Result;
use etcetera::{BaseStrategy, choose_base_strategy};
use serde::Deserialize;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

#[derive(Debug, Deserialize, Default, Clone)]
pub struct Config {
    #[serde(default)]
    pub custom_commands: BTreeMap<String, CustomCommand>,
    /// Path the config was loaded from (for error messages)
    #[serde(skip)]
    pub config_path: Option<PathBuf>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct CustomCommand {
    /// The key(s) to trigger this command (e.g., ["g"])
    pub key: Vec<String>,
    /// The command and arguments to run (e.g., ["lazygit", "log"])
    pub cmd: Vec<String>,
}

impl Config {
    /// Load config from the platform config directory.
    /// Linux/macOS: $XDG_CONFIG_HOME/pui/config.toml (fallback: ~/.config/pui/config.toml)
    /// Windows: %APPDATA%\pui\config.toml
    pub fn load() -> Result<Self> {
        let config_path = choose_base_strategy()?.config_dir().join("pui").join("config.toml");
        Self::load_from_path(&config_path)
    }

    /// Load config from a specific path (useful for testing)
    pub fn load_from_path(path: &Path) -> Result<Self> {
        println!("Loading config from: {}", path.display());
        if path.exists() {
            let content = std::fs::read_to_string(path)?;
            let mut config: Config = toml::from_str(&content)?;
            config.config_path = Some(path.to_path_buf());
            Ok(config)
        } else {
            Ok(Config {
                config_path: Some(path.to_path_buf()),
                ..Config::default()
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_parse_single_command() {
        let toml = r#"
[custom_commands]
lazygit = { key = ["g"], cmd = ["lazygit", "log"] }
"#;
        let config: Config = toml::from_str(toml).unwrap();
        assert_eq!(config.custom_commands.len(), 1);
        let cmd = config.custom_commands.get("lazygit").unwrap();
        assert_eq!(cmd.key, vec!["g"]);
        assert_eq!(cmd.cmd, vec!["lazygit", "log"]);
    }

    #[test]
    fn test_config_parse_multiple_commands() {
        let toml = r#"
[custom_commands]
lazygit = { key = ["g"], cmd = ["lazygit", "log"] }
editor = { key = ["e"], cmd = ["nvim", "."] }
"#;
        let config: Config = toml::from_str(toml).unwrap();
        assert_eq!(config.custom_commands.len(), 2);
    }

    #[test]
    fn test_config_parse_empty() {
        let toml = "";
        let config: Config = toml::from_str(toml).unwrap();
        assert!(config.custom_commands.is_empty());
    }

    #[test]
    fn test_config_missing_file_returns_default() {
        let config = Config::load_from_path(Path::new("/nonexistent/config.toml"));
        assert!(config.is_ok());
        assert!(config.unwrap().custom_commands.is_empty());
    }
}
