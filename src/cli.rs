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
        /// Specific package manager to check
        package_manager: Option<String>,

        /// Show what would be done without actually doing it
        #[arg(long)]
        dry_run: bool,
    },

    /// Perform updates
    Update {
        /// Specific package manager to update
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
}

impl Cli {
    pub fn get_config_path(&self) -> PathBuf {
        if let Some(config_path) = &self.config {
            config_path.clone()
        } else {
            // 홈 디렉토리 기반 설정 파일 경로
            if let Some(home_dir) = dirs::home_dir() {
                let config_path = home_dir.join(".config").join("updog").join("updog.yaml");
                if config_path.exists() {
                    return config_path;
                }
            }

            // XDG_CONFIG_HOME 기반 설정 파일 경로
            if let Some(config_dir) = dirs::config_dir() {
                let config_path = config_dir.join("updog").join("updog.yaml");
                if config_path.exists() {
                    return config_path;
                }
            }

            // 기본 설정 파일 경로 (홈 디렉토리 기준)
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
            command: Commands::Check {
                package_manager: None,
                dry_run: false,
            },
        };
        assert_eq!(cli.get_config_path(), PathBuf::from("custom.yaml"));
    }

    #[test]
    fn test_config_path_default() {
        let cli = Cli {
            config: None,
            verbose: false,
            command: Commands::Check {
                package_manager: None,
                dry_run: false,
            },
        };

        let config_path = cli.get_config_path();
        assert!(config_path
            .to_string_lossy()
            .contains(".config/updog/updog.yaml"));
    }
}
