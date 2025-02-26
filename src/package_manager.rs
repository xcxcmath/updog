use crate::config::Config;
use std::error::Error;
use std::fmt;
use std::process::{Command, ExitStatus, Stdio};
use tracing::{info, warn};

#[derive(Debug)]
pub struct UpdateError {
    pub message: String,
}

impl fmt::Display for UpdateError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Update error: {}", self.message)
    }
}

impl Error for UpdateError {}

pub struct PackageManager {
    pub config: Config,
    pub dry_run: bool,
}

impl PackageManager {
    fn run_command(&self, command: &str) -> Result<ExitStatus, UpdateError> {
        // Dry run mode - just log the command without executing it
        if self.dry_run {
            info!("[DRY RUN] Would execute: {}", command);
            // Instead of using from_raw, execute a simple "true" command
            // which will always succeed with exit code 0
            return Command::new("true").status().map_err(|e| UpdateError {
                message: format!("Failed to execute command: {}", e),
            });
        }

        let status = Command::new("sh")
            .arg("-c")
            .arg(command)
            .stdin(Stdio::inherit())
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .status()
            .map_err(|e| UpdateError {
                message: format!("Failed to execute command: {}", e),
            })?;

        // Print exit code
        if let Some(code) = status.code() {
            if code == 0 {
                info!("Command completed with exit code: {}", code);
            } else {
                warn!("Command completed with non-zero exit code: {}", code);
            }
        } else {
            warn!("Command terminated by signal");
        }

        Ok(status)
    }

    pub fn new(config: Config) -> Self {
        Self {
            config,
            dry_run: false,
        }
    }

    pub fn with_dry_run(config: Config, dry_run: bool) -> Self {
        Self { config, dry_run }
    }

    pub fn with_default_config() -> Self {
        Self {
            config: Config::default(),
            dry_run: false,
        }
    }

    pub fn check(&self, manager_name: &str) -> Result<(), UpdateError> {
        let command = self
            .config
            .commands
            .get(manager_name)
            .ok_or_else(|| UpdateError {
                message: format!("Unknown package manager: {}", manager_name),
            })?;

        // If check command is not defined, print a message and return Ok
        let check_cmd = match &command.check {
            Some(cmd) => cmd,
            None => {
                info!("No check command defined for {}", manager_name);
                return Ok(());
            }
        };

        info!("Running check command for {}: {}", manager_name, check_cmd);
        let status = self.run_command(check_cmd)?;

        if !status.success() {
            return Err(UpdateError {
                message: format!("Check command failed with exit code: {}", status),
            });
        }

        Ok(())
    }

    pub fn update(&self, manager_name: &str) -> Result<(), UpdateError> {
        let command = self
            .config
            .commands
            .get(manager_name)
            .ok_or_else(|| UpdateError {
                message: format!("Unknown package manager: {}", manager_name),
            })?;

        // If update command is not defined, return an error
        let update_cmd = match &command.update {
            Some(cmd) => cmd,
            None => {
                return Err(UpdateError {
                    message: format!("No update command defined for {}", manager_name),
                });
            }
        };

        info!(
            "Running update command for {}: {}",
            manager_name, update_cmd
        );
        let status = self.run_command(update_cmd)?;

        if !status.success() {
            return Err(UpdateError {
                message: format!("Update command failed with exit code: {}", status),
            });
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::UpdateCommand;
    use std::collections::HashMap;

    fn create_test_config() -> Config {
        let mut commands = HashMap::new();
        commands.insert(
            "test".to_string(),
            UpdateCommand {
                check: Some("echo 'updates available'".to_string()),
                update: Some("echo 'Updated'".to_string()),
            },
        );
        commands.insert(
            "no_check".to_string(),
            UpdateCommand {
                check: None,
                update: Some("echo 'Updated without check'".to_string()),
            },
        );
        commands.insert(
            "no_update".to_string(),
            UpdateCommand {
                check: Some("echo 'Cannot update'".to_string()),
                update: None,
            },
        );
        commands.insert(
            "fail".to_string(),
            UpdateCommand {
                check: Some("exit 1".to_string()),
                update: Some("exit 2".to_string()),
            },
        );
        Config { commands }
    }

    #[test]
    fn test_package_manager_check() {
        let config = create_test_config();
        let pm = PackageManager::new(config);

        assert!(pm.check("test").is_ok());
    }

    #[test]
    fn test_package_manager_update() {
        let config = create_test_config();
        let pm = PackageManager::new(config);

        assert!(pm.update("test").is_ok());
    }

    #[test]
    fn test_package_manager_no_check_command() {
        let config = create_test_config();
        let pm = PackageManager::new(config);

        // If check command is not defined, it is not an error
        assert!(pm.check("no_check").is_ok());
    }

    #[test]
    fn test_package_manager_no_update_command() {
        let config = create_test_config();
        let pm = PackageManager::new(config);

        // If update command is not defined, it is an error
        assert!(pm.update("no_update").is_err());
    }

    #[test]
    fn test_failed_command() {
        let config = create_test_config();
        let pm = PackageManager::new(config);

        // If the command fails, it is an error
        assert!(pm.check("fail").is_err());
        assert!(pm.update("fail").is_err());
    }

    #[test]
    fn test_unknown_package_manager() {
        let pm = PackageManager::with_default_config();
        let result = pm.check("unknown");
        assert!(result.is_err());
    }

    #[test]
    fn test_update_error_display() {
        let error = UpdateError {
            message: "test error".to_string(),
        };
        assert_eq!(error.to_string(), "Update error: test error");
    }
}
