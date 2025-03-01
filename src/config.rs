use serde::{Deserialize, Deserializer, Serialize};
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

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PackageManagerConfig {
    pub id: String,
    #[serde(default)]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub subcommands: Vec<SubcommandConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub check: Option<CommandSequence>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub update: Option<CommandSequence>,
}

impl PackageManagerConfig {
    // Find a subcommand with the specified ID
    pub fn find_subcommand(&self, id: &str) -> Option<&SubcommandConfig> {
        self.subcommands.iter().find(|sc| sc.id == id)
    }
    
    // Get the default subcommand (either "default" or the first one)
    pub fn default_subcommand(&self) -> Option<&SubcommandConfig> {
        let default_sc = self.find_subcommand("default");
        if default_sc.is_some() {
            return default_sc;
        }
        
        if !self.subcommands.is_empty() {
            return Some(&self.subcommands[0]);
        }
        
        if self.check.is_some() || self.update.is_some() {
            return None;
        }
        
        None
    }
    
    // Find UpdateCommand including subcommand or direct commands (for backward compatibility)
    pub fn find_subcommand_command(&self, subcommand_id: Option<&str>) -> Option<UpdateCommand> {
        match subcommand_id {
            Some(sc_id) => self.find_subcommand(sc_id).map(|sc| sc.command.clone()),
            
            None => {
                if let Some(sc) = self.default_subcommand() {
                    return Some(sc.command.clone());
                }
                
                if self.check.is_some() || self.update.is_some() {
                    return Some(UpdateCommand {
                        check: self.check.clone(),
                        update: self.update.clone(),
                    });
                }
                
                None
            }
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SubcommandConfig {
    pub id: String,
    #[serde(flatten)]
    pub command: UpdateCommand,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(untagged)]
pub enum CommandSequence {
    Single(String),
    Multiple(Vec<String>),
}

impl CommandSequence {
    pub fn as_single_str(&self) -> Option<&str> {
        match self {
            CommandSequence::Single(s) => Some(s),
            _ => None,
        }
    }
    
    pub fn as_multiple(&self) -> Option<&Vec<String>> {
        match self {
            CommandSequence::Multiple(v) => Some(v),
            _ => None,
        }
    }
}

#[derive(Debug, Serialize, Clone)]
pub struct UpdateCommand {
    pub check: Option<CommandSequence>,
    pub update: Option<CommandSequence>,
}

impl<'de> Deserialize<'de> for UpdateCommand {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = serde_yaml::Value::deserialize(deserializer)?;

        match value {
            serde_yaml::Value::Mapping(map) => {
                let mut check = None;
                let mut update = None;

                if let Some(check_val) = map.get(&serde_yaml::Value::String("check".to_string())) {
                    if let serde_yaml::Value::String(s) = check_val {
                        check = Some(CommandSequence::Single(s.clone()));
                    } else if let serde_yaml::Value::Sequence(seq) = check_val {
                        let commands: Result<Vec<String>, _> = seq
                            .iter()
                            .map(|val| {
                                if let serde_yaml::Value::String(s) = val {
                                    Ok(s.clone())
                                } else {
                                    Err(serde::de::Error::custom(
                                        "Expected string in command sequence",
                                    ))
                                }
                            })
                            .collect();
                        
                        check = Some(CommandSequence::Multiple(commands?));
                    }
                }

                if let Some(update_val) = map.get(&serde_yaml::Value::String("update".to_string()))
                {
                    if let serde_yaml::Value::String(s) = update_val {
                        update = Some(CommandSequence::Single(s.clone()));
                    } else if let serde_yaml::Value::Sequence(seq) = update_val {
                        let commands: Result<Vec<String>, _> = seq
                            .iter()
                            .map(|val| {
                                if let serde_yaml::Value::String(s) = val {
                                    Ok(s.clone())
                                } else {
                                    Err(serde::de::Error::custom(
                                        "Expected string in command sequence",
                                    ))
                                }
                            })
                            .collect();
                        
                        update = Some(CommandSequence::Multiple(commands?));
                    }
                }

                Ok(UpdateCommand { check, update })
            }

            _ => Err(serde::de::Error::custom(
                "Expected mapping for UpdateCommand",
            )),
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Config {
    pub commands: Vec<PackageManagerConfig>,
}

impl Config {
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self, ConfigError> {
        let content = fs::read_to_string(path)?;
        let config = serde_yaml::from_str(&content)?;
        Ok(config)
    }

    pub fn default() -> Self {
        Self {
            commands: vec![
                PackageManagerConfig {
                    id: "homebrew".to_string(),
                    subcommands: vec![
                        SubcommandConfig {
                            id: "default".to_string(),
                            command: UpdateCommand {
                                check: Some(CommandSequence::Single("brew outdated".to_string())),
                                update: Some(CommandSequence::Single("brew upgrade".to_string())),
                            },
                        },
                    ],
                    check: None,
                    update: None,
                },
            ],
        }
    }

    pub fn find_package_manager(&self, id: &str) -> Option<&PackageManagerConfig> {
        self.commands.iter().find(|pm| pm.id == id)
    }
    
    // Find a subcommand for a specific package manager
    pub fn find_subcommand(&self, manager_id: &str, subcommand_id: Option<&str>) -> Option<SubcommandConfig> {
        let manager = self.find_package_manager(manager_id)?;
        
        if let Some(sc_id) = subcommand_id {
            if let Some(sc) = manager.find_subcommand(sc_id) {
                return Some(sc.clone());
            }
            return None;
        }
        
        if let Some(sc) = manager.default_subcommand() {
            return Some(sc.clone());
        }
        
        if manager.check.is_some() || manager.update.is_some() {
            return Some(SubcommandConfig {
                id: "default".to_string(),
                command: UpdateCommand {
                    check: manager.check.clone(),
                    update: manager.update.clone(),
                },
            });
        }
        
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = Config::default();
        assert_eq!(config.commands.len(), 1);
        let homebrew = &config.commands[0];
        assert_eq!(homebrew.id, "homebrew");
        
        // Test subcommands
        assert_eq!(homebrew.subcommands.len(), 1);
        let default_sc = homebrew.default_subcommand().unwrap();
        assert_eq!(default_sc.id, "default");
        
        let cmd = &default_sc.command;
        assert!(cmd.check.is_some());
        assert!(cmd.update.is_some());
    }

    #[test]
    fn test_find_package_manager() {
        let config = Config::default();
        
        let homebrew = config.find_package_manager("homebrew");
        assert!(homebrew.is_some());
        assert_eq!(homebrew.unwrap().id, "homebrew");
        
        let unknown = config.find_package_manager("unknown");
        assert!(unknown.is_none());
    }

    #[test]
    fn test_parse_valid_config() {
        let yaml = r#"
        commands:
          - id: homebrew
            subcommands:
              - id: default
                check: brew outdated
                update: brew upgrade
          - id: npm
            subcommands:
              - id: default
                update: npm update -g
        "#;

        let config: Config = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(config.commands.len(), 2);
        
        let homebrew = config.find_package_manager("homebrew").unwrap();
        let homebrew_default = homebrew.default_subcommand().unwrap();
        assert!(homebrew_default.command.check.is_some());
        
        let npm = config.find_package_manager("npm").unwrap();
        let npm_default = npm.default_subcommand().unwrap();
        assert!(npm_default.command.update.is_some());
        assert!(npm_default.command.check.is_none());
    }

    #[test]
    fn test_parse_command_sequence() {
        let yaml = r#"
        commands:
          - id: npm
            subcommands:
              - id: default
                update:
                  - npm cache clean -f
                  - npm update -g
          - id: rust
            subcommands:
              - id: default
                update: rustup update
        "#;

        let config: Config = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(config.commands.len(), 2);
        
        let npm = config.find_package_manager("npm").unwrap();
        let npm_default = npm.default_subcommand().unwrap();
        let npm_update = &npm_default.command.update;
        assert!(npm_update.is_some());
        
        let commands = npm_update.as_ref().unwrap().as_multiple().unwrap();
        assert_eq!(commands.len(), 2);
        assert_eq!(commands[0], "npm cache clean -f");
        assert_eq!(commands[1], "npm update -g");
        
        let rust = config.find_package_manager("rust").unwrap();
        let rust_default = rust.default_subcommand().unwrap();
        let rust_update = &rust_default.command.update;
        assert!(rust_update.is_some());
        assert_eq!(rust_update.as_ref().unwrap().as_single_str().unwrap(), "rustup update");
    }

    #[test]
    fn test_order_preservation() {
        let yaml = r#"
        commands:
          - id: brew
            subcommands:
              - id: default
                update: brew upgrade
          - id: npm
            subcommands:
              - id: default
                update: npm update -g
        "#;

        let config: Config = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(config.commands[0].id, "brew");
        assert_eq!(config.commands[1].id, "npm");
    }
    
    #[test]
    fn test_parse_subcommands() {
        let yaml = r#"
        commands:
          - id: rustup
            subcommands:
              - id: default
                check: rustup check
                update: rustup update
              - id: self
                update: rustup self update
        "#;

        let config: Config = serde_yaml::from_str(yaml).unwrap();
        let rustup = config.find_package_manager("rustup").unwrap();
        
        // Test default subcommand
        let default_sc = rustup.default_subcommand().unwrap();
        assert_eq!(default_sc.id, "default");
        assert!(default_sc.command.check.is_some());
        
        // Test specific subcommand
        let self_sc = rustup.find_subcommand("self").unwrap();
        assert_eq!(self_sc.id, "self");
        assert!(self_sc.command.update.is_some());
        assert!(self_sc.command.check.is_none());
        
        // Test find_subcommand method
        let self_sc2 = config.find_subcommand("rustup", Some("self")).unwrap();
        assert_eq!(self_sc2.id, "self");
        
        // Test default subcommand
        let default_sc2 = config.find_subcommand("rustup", None).unwrap();
        assert_eq!(default_sc2.id, "default");
    }

    #[test]
    fn test_parse_subcommands_with_simple_fields() {
        let yaml = r#"
        commands:
          - id: npm
            subcommands:
              - id: default
                update: npm update -g
              - id: globals
                update: npm update -g
                check: npm outdated -g
          - id: other
            subcommands:
              - id: default
                check: other check
        "#;

        let config: Config = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(config.commands.len(), 2);
        
        let npm = config.find_package_manager("npm").unwrap();
        assert_eq!(npm.subcommands.len(), 2);
        
        let npm_default = npm.default_subcommand().unwrap();
        assert_eq!(npm_default.id, "default");
        assert!(npm_default.command.update.is_some());
        assert!(npm_default.command.check.is_none());
    }

    #[test]
    fn test_parse_command_sequence_with_simple_fields() {
        let yaml = r#"
        commands:
          - id: npm
            subcommands:
              - id: default
                update:
                  - npm cache clean -f
                  - npm update -g
          - id: rust
            subcommands:
              - id: default
                update: rustup update
        "#;

        let config: Config = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(config.commands.len(), 2);
        
        let npm = config.find_package_manager("npm").unwrap();
        let npm_default = npm.default_subcommand().unwrap();
        let npm_update = &npm_default.command.update;
        assert!(npm_update.is_some());
        
        let commands = npm_update.as_ref().unwrap().as_multiple().unwrap();
        assert_eq!(commands.len(), 2);
        assert_eq!(commands[0], "npm cache clean -f");
        assert_eq!(commands[1], "npm update -g");
        
        let rust = config.find_package_manager("rust").unwrap();
        let rust_default = rust.default_subcommand().unwrap();
        let rust_update = &rust_default.command.update;
        assert!(rust_update.is_some());
        assert_eq!(rust_update.as_ref().unwrap().as_single_str().unwrap(), "rustup update");
    }

    #[test]
    fn test_simple_config_compatibility() {
        // Simple format config file (with direct check/update fields)
        let yaml = r#"
        commands:
          - id: brew
            check: brew outdated
            update: brew upgrade
          - id: mixed
            check: mixed check
            update: mixed update
            subcommands:
              - id: sub1
                check: sub1 check
        "#;

        let config: Config = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(config.commands.len(), 2);
        
        // Test package manager with direct fields only
        let brew = config.find_package_manager("brew").unwrap();
        assert_eq!(brew.subcommands.len(), 0);
        assert!(brew.check.is_some());
        assert!(brew.update.is_some());
        
        // Test if find_subcommand properly handles direct fields
        let brew_subcommand = config.find_subcommand("brew", None).unwrap();
        assert_eq!(brew_subcommand.id, "default");
        assert_eq!(
            brew_subcommand.command.check.as_ref().unwrap().as_single_str().unwrap(),
            "brew outdated"
        );
        assert_eq!(
            brew_subcommand.command.update.as_ref().unwrap().as_single_str().unwrap(),
            "brew upgrade"
        );
        
        // Test package manager with both direct fields and subcommands
        let mixed = config.find_package_manager("mixed").unwrap();
        assert_eq!(mixed.subcommands.len(), 1);
        assert!(mixed.check.is_some());
        assert!(mixed.update.is_some());
        
        // Default subcommand (subcommands have priority)
        let mixed_default = config.find_subcommand("mixed", None).unwrap();
        assert_eq!(mixed_default.id, "sub1");
        assert_eq!(
            mixed_default.command.check.as_ref().unwrap().as_single_str().unwrap(),
            "sub1 check"
        );
        
        // Specific subcommand request
        let mixed_sub1 = config.find_subcommand("mixed", Some("sub1")).unwrap();
        assert_eq!(mixed_sub1.id, "sub1");
        
        // Direct fields should not be returned when requesting a nonexistent subcommand
        let nonexistent = config.find_subcommand("mixed", Some("nonexistent"));
        assert!(nonexistent.is_none());
    }
}
