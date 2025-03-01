use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(author, version, about)]
pub struct Cli {
    /// Optional configuration file path
    #[arg(short, long, value_name = "FILE")]
    pub config: Option<PathBuf>,

    /// Enable verbose logging
    #[arg(short, long)]
    pub verbose: bool,

    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Check for available updates
    Check {
        /// Specific package manager to check (format: manager[:subcommand])
        package_manager: Option<String>,

        /// Show what would be done without actually doing it
        #[arg(long)]
        dry_run: bool,
    },

    /// Perform updates
    Update {
        /// Specific package manager to update (format: manager[:subcommand])
        package_manager: Option<String>,

        /// Show what would be done without actually doing it
        #[arg(long)]
        dry_run: bool,
    },

    /// Launch TUI mode
    Tui,
}

impl Commands {
    // Returns whether the command is in dry run mode
    pub fn is_dry_run(&self) -> bool {
        match self {
            Commands::Check { dry_run, .. } => *dry_run,
            Commands::Update { dry_run, .. } => *dry_run,
            Commands::Tui => false,
        }
    }

    // Parse package manager string to extract package manager and subcommand
    // Format: manager[:subcommand]
    pub fn parse_package_manager(&self) -> Option<(String, Option<String>)> {
        let package_manager = match self {
            Commands::Check { package_manager, .. } => package_manager,
            Commands::Update { package_manager, .. } => package_manager,
            Commands::Tui => return None,
        };

        package_manager.as_ref().map(|pm_str| {
            // Split by ':' to get package manager and subcommand
            let parts: Vec<&str> = pm_str.split(':').collect();
            match parts.len() {
                1 => (parts[0].to_string(), None),
                2 => (parts[0].to_string(), Some(parts[1].to_string())),
                _ => {
                    // If there are more than one ':', take the first part as package manager
                    // and the rest joined by ':' as subcommand
                    let manager = parts[0].to_string();
                    let subcommand = parts[1..].join(":").to_string();
                    (manager, Some(subcommand))
                }
            }
        })
    }
}

impl Cli {
    pub fn get_config_path(&self) -> PathBuf {
        if let Some(config_path) = &self.config {
            config_path.clone()
        } else {
            // Home directory based configuration file path
            if let Some(home_dir) = dirs::home_dir() {
                let config_path = home_dir.join(".config").join("updog").join("updog.yaml");
                if config_path.exists() {
                    return config_path;
                }
            }

            // XDG_CONFIG_HOME based configuration file path
            if let Some(config_dir) = dirs::config_dir() {
                let config_path = config_dir.join("updog").join("updog.yaml");
                if config_path.exists() {
                    return config_path;
                }
            }

            // Default configuration file path (based on home directory)
            dirs::home_dir()
                .unwrap_or_else(|| PathBuf::from("."))
                .join(".config")
                .join("updog")
                .join("updog.yaml")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verify_cli() {
        use clap::CommandFactory;
        Cli::command().debug_assert()
    }

    #[test]
    fn test_config_path_with_custom_path() {
        let cli = Cli {
            config: Some(PathBuf::from("custom.yaml")),
            verbose: false,
            command: Commands::Tui,
        };
        assert_eq!(cli.get_config_path(), PathBuf::from("custom.yaml"));
    }

    #[test]
    fn test_config_path_default() {
        // This test just verifies that the function doesn't panic
        // when no custom path is provided
        let cli = Cli {
            config: None,
            verbose: false,
            command: Commands::Tui,
        };
        let _path = cli.get_config_path();
    }

    #[test]
    fn test_package_manager_parsing() {
        // Test case 1: Just package manager name
        let cmd = Commands::Check {
            package_manager: Some("brew".to_string()),
            dry_run: false,
        };
        let result = cmd.parse_package_manager();
        assert_eq!(result, Some(("brew".to_string(), None)));

        // Test case 2: Package manager with subcommand
        let cmd = Commands::Update {
            package_manager: Some("brew:cask".to_string()),
            dry_run: false,
        };
        let result = cmd.parse_package_manager();
        assert_eq!(result, Some(("brew".to_string(), Some("cask".to_string()))));

        // Test case 3: Package manager with complex subcommand (containing ':')
        let cmd = Commands::Check {
            package_manager: Some("custom:with:colons".to_string()),
            dry_run: false,
        };
        let result = cmd.parse_package_manager();
        assert_eq!(result, Some(("custom".to_string(), Some("with:colons".to_string()))));

        // Test case 4: No package manager specified
        let cmd = Commands::Check {
            package_manager: None,
            dry_run: false,
        };
        let result = cmd.parse_package_manager();
        assert_eq!(result, None);

        // Test case 5: TUI mode has no package manager
        let cmd = Commands::Tui;
        let result = cmd.parse_package_manager();
        assert_eq!(result, None);
    }
}
