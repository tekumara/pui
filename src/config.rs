use anyhow::{Context, Result};
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
    /// The key to trigger this command (e.g., "g", "ctrl+g", "alt+r", "opt+q")
    pub key: String,
    /// The command and arguments to run (e.g., ["lazygit", "log"])
    pub cmd: Vec<String>,
}

/// Parsed key binding with optional modifiers
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedKey {
    pub key: char,
    pub ctrl: bool,
    pub alt: bool,
}

impl ParsedKey {
    /// Parse a key string like "g", "ctrl+g", "alt+r", "opt+q"
    pub fn parse(s: &str) -> Option<Self> {
        let s = s.trim().to_lowercase();
        let parts: Vec<&str> = s.split('+').collect();

        match parts.as_slice() {
            [key] if key.len() == 1 => Some(ParsedKey {
                key: key.chars().next()?,
                ctrl: false,
                alt: false,
            }),
            [modifier, key] if key.len() == 1 => {
                let key_char = key.chars().next()?;
                match *modifier {
                    "ctrl" => Some(ParsedKey {
                        key: key_char,
                        ctrl: true,
                        alt: false,
                    }),
                    "alt" | "opt" => Some(ParsedKey {
                        key: key_char,
                        ctrl: false,
                        alt: true,
                    }),
                    _ => None,
                }
            }
            _ => None,
        }
    }

    /// Check if this parsed key matches a crossterm KeyEvent
    pub fn matches(&self, key_event: &crossterm::event::KeyEvent) -> bool {
        use crossterm::event::{KeyCode, KeyModifiers};

        let code_matches = match key_event.code {
            KeyCode::Char(c) => c.to_ascii_lowercase() == self.key,
            _ => false,
        };

        let ctrl_matches = key_event.modifiers.contains(KeyModifiers::CONTROL) == self.ctrl;
        let alt_matches = key_event.modifiers.contains(KeyModifiers::ALT) == self.alt;

        code_matches && ctrl_matches && alt_matches
    }
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
        if path.exists() {
            let content = std::fs::read_to_string(path)
                .with_context(|| format!("Failed to read config file {}", path.display()))?;
            let mut config: Config = toml::from_str(&content)
                .with_context(|| format!("Failed to parse config file {}", path.display()))?;
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
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    #[test]
    fn test_config_parse_single_command() {
        let toml = r#"
[custom_commands]
lazygit = { key = "g", cmd = ["lazygit", "log"] }
"#;
        let config: Config = toml::from_str(toml).unwrap();
        assert_eq!(config.custom_commands.len(), 1);
        let cmd = config.custom_commands.get("lazygit").unwrap();
        assert_eq!(cmd.key, "g");
        assert_eq!(cmd.cmd, vec!["lazygit", "log"]);
    }

    #[test]
    fn test_config_parse_multiple_commands() {
        let toml = r#"
[custom_commands]
lazygit = { key = "g", cmd = ["lazygit", "log"] }
editor = { key = "ctrl+e", cmd = ["nvim", "."] }
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

    #[test]
    fn test_parsed_key_simple() {
        let pk = ParsedKey::parse("g").unwrap();
        assert_eq!(pk.key, 'g');
        assert!(!pk.ctrl);
        assert!(!pk.alt);
    }

    #[test]
    fn test_parsed_key_ctrl() {
        let pk = ParsedKey::parse("ctrl+p").unwrap();
        assert_eq!(pk.key, 'p');
        assert!(pk.ctrl);
        assert!(!pk.alt);
    }

    #[test]
    fn test_parsed_key_alt() {
        let pk = ParsedKey::parse("alt+r").unwrap();
        assert_eq!(pk.key, 'r');
        assert!(!pk.ctrl);
        assert!(pk.alt);
    }

    #[test]
    fn test_parsed_key_opt() {
        let pk = ParsedKey::parse("opt+q").unwrap();
        assert_eq!(pk.key, 'q');
        assert!(!pk.ctrl);
        assert!(pk.alt); // opt is alias for alt
    }

    #[test]
    fn test_parsed_key_matches_simple() {
        let pk = ParsedKey::parse("g").unwrap();
        let key_event = KeyEvent::new(KeyCode::Char('g'), KeyModifiers::NONE);
        assert!(pk.matches(&key_event));

        // Should not match with modifiers
        let key_event_ctrl = KeyEvent::new(KeyCode::Char('g'), KeyModifiers::CONTROL);
        assert!(!pk.matches(&key_event_ctrl));
    }

    #[test]
    fn test_parsed_key_matches_ctrl() {
        let pk = ParsedKey::parse("ctrl+p").unwrap();
        let key_event = KeyEvent::new(KeyCode::Char('p'), KeyModifiers::CONTROL);
        assert!(pk.matches(&key_event));

        // Should not match without modifier
        let key_event_no_mod = KeyEvent::new(KeyCode::Char('p'), KeyModifiers::NONE);
        assert!(!pk.matches(&key_event_no_mod));
    }

    #[test]
    fn test_parsed_key_matches_alt() {
        let pk = ParsedKey::parse("alt+r").unwrap();
        let key_event = KeyEvent::new(KeyCode::Char('r'), KeyModifiers::ALT);
        assert!(pk.matches(&key_event));
    }

    #[test]
    fn test_parsed_key_case_insensitive() {
        let pk = ParsedKey::parse("CTRL+P").unwrap();
        assert_eq!(pk.key, 'p');
        assert!(pk.ctrl);
    }

    #[test]
    fn test_parsed_key_invalid() {
        assert!(ParsedKey::parse("").is_none());
        assert!(ParsedKey::parse("ctrl+").is_none());
        assert!(ParsedKey::parse("invalid+g").is_none());
        assert!(ParsedKey::parse("ctrl+ab").is_none()); // multi-char key
    }
}
