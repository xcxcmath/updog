use serde::{Deserialize, Deserializer, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("Failed to read config file: {0}")]
    FileReadError(#[from] std::io::Error),
    #[error("Failed to parse config file: {0}")]
    ParseError(#[from] serde_yaml::Error),
}

#[derive(Debug, Serialize)]
pub struct UpdateCommand {
    pub check: Option<String>,
    pub update: Option<String>,
}

impl<'de> Deserialize<'de> for UpdateCommand {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        // First, parse the value as serde_yaml::Value
        let value = serde_yaml::Value::deserialize(deserializer)?;

        match value {
            // String values are treated as update commands
            serde_yaml::Value::String(s) => Ok(UpdateCommand {
                check: None,
                update: Some(s),
            }),

            // Mappings (objects) are parsed in the usual way
            serde_yaml::Value::Mapping(map) => {
                let mut check = None;
                let mut update = None;

                // Check if the "check" key exists
                if let Some(check_val) = map.get(&serde_yaml::Value::String("check".to_string())) {
                    if let serde_yaml::Value::String(s) = check_val {
                        check = Some(s.clone());
                    }
                }

                // Check if the "update" key exists
                if let Some(update_val) = map.get(&serde_yaml::Value::String("update".to_string()))
                {
                    if let serde_yaml::Value::String(s) = update_val {
                        update = Some(s.clone());
                    }
                }

                Ok(UpdateCommand { check, update })
            }

            // Other types are errors
            _ => Err(serde::de::Error::custom(
                "Expected string or mapping for UpdateCommand",
            )),
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Config {
    pub commands: HashMap<String, UpdateCommand>,
}

impl Config {
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self, ConfigError> {
        let contents = fs::read_to_string(path)?;
        let config = serde_yaml::from_str(&contents)?;
        Ok(config)
    }

    pub fn default() -> Self {
        let mut commands = HashMap::new();
        commands.insert(
            "homebrew".to_string(),
            UpdateCommand {
                check: Some("brew outdated".to_string()),
                update: Some("brew upgrade".to_string()),
            },
        );
        Self { commands }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = Config::default();
        assert!(config.commands.contains_key("homebrew"));
        let homebrew = config.commands.get("homebrew").unwrap();
        assert_eq!(homebrew.check, Some("brew outdated".to_string()));
        assert_eq!(homebrew.update, Some("brew upgrade".to_string()));
    }

    #[test]
    fn test_parse_valid_config() {
        let yaml = r#"
        commands:
          homebrew:
            check: "brew outdated"
            update: "brew upgrade"
          apt:
            check: "apt list --upgradable"
            update: "apt upgrade -y"
        "#;
        let config: Config = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(config.commands.len(), 2);
        assert!(config.commands.contains_key("homebrew"));
        assert!(config.commands.contains_key("apt"));
    }

    #[test]
    fn test_parse_string_as_update_command() {
        let yaml = r#"
        commands:
          homebrew:
            check: "brew outdated"
            update: "brew upgrade"
          npm: "npm update -g"
        "#;
        let config: Config = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(config.commands.len(), 2);

        let npm = config.commands.get("npm").unwrap();
        assert_eq!(npm.check, None);
        assert_eq!(npm.update, Some("npm update -g".to_string()));
    }
}
