use std::process::{Command, ExitStatus, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use tracing::{error, info};

use crate::config::{CommandSequence, Config};

#[derive(Debug)]
pub struct UpdateError {
    pub message: String,
}

impl std::fmt::Display for UpdateError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "Update error: {}", self.message)
    }
}

impl std::error::Error for UpdateError {}

pub struct PackageManager {
    pub config: Config,
    pub dry_run: bool,
    process_tracker: Arc<Mutex<ProcessTracker>>,
}

// Structure to track running processes
struct ProcessTracker {
    active_processes: std::collections::HashSet<u32>, // Set of active process IDs
    shutdown_requested: Arc<AtomicBool>,
}

impl ProcessTracker {
    fn new() -> Self {
        Self {
            active_processes: std::collections::HashSet::new(),
            shutdown_requested: Arc::new(AtomicBool::new(false)),
        }
    }

    // Register a process
    fn register_process(&mut self, pid: u32) {
        self.active_processes.insert(pid);
    }

    // Unregister a process
    fn unregister_process(&mut self, pid: u32) {
        self.active_processes.remove(&pid);
    }

    // Mark shutdown as requested
    fn request_shutdown(&self) {
        self.shutdown_requested.store(true, Ordering::SeqCst);
    }

    // Check if shutdown has been requested
    fn is_shutdown_requested(&self) -> bool {
        self.shutdown_requested.load(Ordering::SeqCst)
    }

    // Terminate all active processes
    fn terminate_all_processes(&self) {
        for &pid in &self.active_processes {
            // Attempt to send SIGTERM signal
            #[cfg(unix)]
            unsafe {
                libc::kill(pid as i32, libc::SIGTERM);
                info!("Sent SIGTERM to process {}", pid);
            }

            #[cfg(windows)]
            {
                // On Windows, a different termination mechanism is needed
                // Here, we just perform simple logging
                info!(
                    "Process termination on Windows not implemented for PID: {}",
                    pid
                );
            }
        }
    }
}

impl PackageManager {
    // Execute a command sequence (single or multiple commands)
    fn execute_command(
        &self,
        _manager_name: &str,
        command: &CommandSequence,
    ) -> Result<(), UpdateError> {
        match command {
            CommandSequence::Single(cmd) => {
                // Execute a single command
                let status = self.run_single_command(cmd)?;
                if !status.success() {
                    return Err(UpdateError {
                        message: format!("Command failed with exit code: {}", status),
                    });
                }
                Ok(())
            }
            CommandSequence::Multiple(cmds) => {
                // Execute multiple commands in sequence
                for (index, cmd) in cmds.iter().enumerate() {
                    info!("Executing step {} of {}", index + 1, cmds.len());
                    let status = self.run_single_command(cmd)?;
                    if !status.success() {
                        // Stop on first failure and return error
                        return Err(UpdateError {
                            message: format!("Command failed with exit code: {}", status),
                        });
                    }
                }
                Ok(())
            }
        }
    }

    // Execute a single command
    fn run_single_command(&self, command: &str) -> Result<ExitStatus, UpdateError> {
        if self.dry_run {
            info!("Dry run: would execute command: {}", command);
            return Ok(ExitStatus::default()); // Simulate success in dry run mode
        }

        let shell = if cfg!(target_os = "windows") {
            "cmd"
        } else {
            "bash"
        };

        let shell_arg = if cfg!(target_os = "windows") {
            "/C"
        } else {
            "-c"
        };

        info!("Executing command: {}", command);

        // Launch the command
        let mut process = match Command::new(shell)
            .arg(shell_arg)
            .arg(command)
            .stderr(Stdio::inherit())
            .stdout(Stdio::inherit())
            .spawn()
        {
            Ok(p) => p,
            Err(e) => {
                return Err(UpdateError {
                    message: format!("Failed to execute command: {}", e),
                });
            }
        };

        // Get the process ID for tracking
        let pid = process.id();

        // Register the process with the tracker
        {
            let mut tracker = self.process_tracker.lock().unwrap();
            tracker.register_process(pid);

            // Check if shutdown was requested before we even started
            if tracker.is_shutdown_requested() {
                drop(tracker); // Release the lock before terminating

                // If so, terminate immediately
                #[cfg(unix)]
                unsafe {
                    libc::kill(pid as i32, libc::SIGTERM);
                    info!("Terminated process {} due to shutdown request", pid);
                }

                return Err(UpdateError {
                    message: String::from("Operation was cancelled"),
                });
            }
        }

        // Wait for the process to complete
        let exit_status = match process.wait() {
            Ok(status) => status,
            Err(e) => {
                return Err(UpdateError {
                    message: format!("Failed to wait for command: {}", e),
                });
            }
        };

        // Unregister the process when it's done
        {
            let mut tracker = self.process_tracker.lock().unwrap();
            tracker.unregister_process(pid);
        }

        // Check the exit status
        if exit_status.success() {
            info!("Command completed successfully");
        } else {
            error!("Command failed with exit code: {}", exit_status);
        }

        Ok(exit_status)
    }

    pub fn new(config: Config) -> Self {
        let process_tracker = Arc::new(Mutex::new(ProcessTracker::new()));
        let pm = Self {
            config,
            dry_run: false,
            process_tracker: process_tracker.clone(),
        };

        Self::setup_signal_handlers(process_tracker);
        pm
    }

    pub fn with_dry_run(config: Config, dry_run: bool) -> Self {
        let process_tracker = Arc::new(Mutex::new(ProcessTracker::new()));
        let pm = Self {
            config,
            dry_run,
            process_tracker: process_tracker.clone(),
        };

        Self::setup_signal_handlers(process_tracker);
        pm
    }

    pub fn with_default_config() -> Self {
        let process_tracker = Arc::new(Mutex::new(ProcessTracker::new()));
        let pm = Self {
            config: Config::default(),
            dry_run: false,
            process_tracker: process_tracker.clone(),
        };

        Self::setup_signal_handlers(process_tracker);
        pm
    }

    #[cfg(unix)]
    // Set up signal handlers (Unix platforms only)
    fn setup_signal_handlers(process_tracker: Arc<Mutex<ProcessTracker>>) {
        use signal_hook::{
            consts::{SIGINT, SIGTERM},
            iterator::Signals,
        };
        use std::thread;

        let mut signals = Signals::new(&[SIGINT, SIGTERM]).unwrap();
        let process_tracker_clone = process_tracker.clone();

        thread::spawn(move || {
            for sig in signals.forever() {
                info!("Received signal: {}", sig);
                let tracker = process_tracker_clone.lock().unwrap();
                tracker.request_shutdown();
                tracker.terminate_all_processes();
                break;
            }
        });
    }

    #[cfg(not(unix))]
    // Simple implementation on Windows
    fn setup_signal_handlers(process_tracker: Arc<Mutex<ProcessTracker>>) {
        // Windows signal handling requires different mechanisms
        // For simplicity, we'll just log that it's not fully implemented
        // but we'll keep the process_tracker so the rest of the code is consistent
        info!("Signal handling on this platform is limited");

        // In a real application, we'd implement proper Ctrl+C handling for Windows
        // using the ctrlc crate or Windows-specific APIs
    }

    // Clean up on exit
    pub fn cleanup(&self) {
        let tracker = self.process_tracker.lock().unwrap();
        tracker.request_shutdown();
        tracker.terminate_all_processes();
    }

    // Execute the check command for a subcommand of a package manager
    pub fn check_with_subcommand(
        &self,
        manager_name: &str,
        subcommand_name: Option<&str>,
    ) -> Result<(), UpdateError> {
        // Find the package manager and subcommand
        let subcommand = self
            .config
            .find_subcommand(manager_name, subcommand_name)
            .ok_or_else(|| {
                let message = match subcommand_name {
                    Some(sc) => format!(
                        "Unknown subcommand '{}' for package manager '{}'",
                        sc, manager_name
                    ),
                    None => format!(
                        "Unknown package manager or default subcommand: {}",
                        manager_name
                    ),
                };
                UpdateError { message }
            })?;

        // Check if the subcommand has a check command
        if let Some(check_cmd) = &subcommand.command.check {
            info!(
                "Checking updates for {}{}...",
                manager_name,
                subcommand_name.map_or("".to_string(), |s| format!(":{}", s))
            );

            // Execute the check command
            self.execute_command(manager_name, check_cmd)
        } else {
            // No check command specified for this subcommand
            let message = format!(
                "No check command specified for {}{}",
                manager_name,
                subcommand_name.map_or("".to_string(), |s| format!(":{}", s))
            );
            Err(UpdateError { message })
        }
    }

    // Execute the update command for a subcommand of a package manager
    pub fn update_with_subcommand(
        &self,
        manager_name: &str,
        subcommand_name: Option<&str>,
    ) -> Result<(), UpdateError> {
        // Find the package manager and subcommand
        let subcommand = self
            .config
            .find_subcommand(manager_name, subcommand_name)
            .ok_or_else(|| {
                let message = match subcommand_name {
                    Some(sc) => format!(
                        "Unknown subcommand '{}' for package manager '{}'",
                        sc, manager_name
                    ),
                    None => format!(
                        "Unknown package manager or default subcommand: {}",
                        manager_name
                    ),
                };
                UpdateError { message }
            })?;

        // Check if the subcommand has an update command
        if let Some(update_cmd) = &subcommand.command.update {
            info!(
                "Updating packages for {}{}...",
                manager_name,
                subcommand_name.map_or("".to_string(), |s| format!(":{}", s))
            );

            // Execute the update command
            self.execute_command(manager_name, update_cmd)
        } else {
            // No update command specified for this subcommand
            let message = format!(
                "No update command specified for {}{}",
                manager_name,
                subcommand_name.map_or("".to_string(), |s| format!(":{}", s))
            );
            Err(UpdateError { message })
        }
    }

    // Check for updates (uses default subcommand)
    pub fn check(&self, manager_name: &str) -> Result<(), UpdateError> {
        self.check_with_subcommand(manager_name, None)
    }

    // Update packages (uses default subcommand)
    pub fn update(&self, manager_name: &str) -> Result<(), UpdateError> {
        self.update_with_subcommand(manager_name, None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Helper function to create a test configuration with subcommands
    fn create_test_config_with_subcommands() -> Config {
        let yaml = r#"
        commands:
          - id: test
            subcommands:
              - id: default
                check: "echo checking step 1"
                update: "echo updating step 1"
              - id: multi
                check: 
                  - "echo checking step 1"
                  - "echo checking step 2"
                update:
                  - "echo updating step 1" 
                  - "echo updating step 2"
              - id: fail
                check: "exit 1"
                update: "exit 1"
        "#;

        serde_yaml::from_str(yaml).unwrap()
    }

    // Helper function to create a test configuration with legacy format (direct check/update fields)
    fn create_test_config_with_simple_format() -> Config {
        let yaml = r#"
        commands:
          - id: simple
            check: "echo simple checking"
            update: "echo simple updating"
          - id: simple_fail
            check: "exit 1"
            update: "exit 1"
          - id: mixed
            check: "echo mixed direct check"
            update: "echo mixed direct update"
            subcommands:
              - id: sub
                check: "echo mixed sub check"
                update: "echo mixed sub update"
        "#;

        serde_yaml::from_str(yaml).unwrap()
    }

    // Simple test config creation function (maintains compatibility with existing tests)
    fn create_test_config() -> Config {
        let yaml = r#"
        commands:
          - id: test
            subcommands:
              - id: default
                check: "echo checking"
                update: "echo updating"
          - id: nocheck
            subcommands:
              - id: default
                update: "echo updating"
          - id: noupdate
            subcommands:
              - id: default
                check: "echo checking"
          - id: fail
            subcommands:
              - id: default
                check: "exit 1"
                update: "exit 1"
        "#;

        serde_yaml::from_str(yaml).unwrap()
    }

    #[test]
    fn test_package_manager_check() {
        let config = create_test_config();
        let pm = PackageManager::new(config);
        let result = pm.check("test");
        assert!(result.is_ok());
    }

    #[test]
    fn test_package_manager_update() {
        let config = create_test_config();
        let pm = PackageManager::new(config);
        let result = pm.update("test");
        assert!(result.is_ok());
    }

    #[test]
    fn test_package_manager_no_check_command() {
        let config = create_test_config();
        let pm = PackageManager::new(config);
        let result = pm.check("nocheck");
        assert!(result.is_err());
    }

    #[test]
    fn test_package_manager_no_update_command() {
        let config = create_test_config();
        let pm = PackageManager::new(config);
        let result = pm.update("noupdate");
        assert!(result.is_err());
    }

    #[test]
    fn test_failed_command() {
        let config = create_test_config();
        let pm = PackageManager::new(config);
        let result = pm.check("fail");
        assert!(result.is_err());
    }

    #[test]
    fn test_unknown_package_manager() {
        let config = create_test_config();
        let pm = PackageManager::new(config);
        let result = pm.check("unknown");
        assert!(result.is_err());
    }

    #[test]
    fn test_update_error_display() {
        let error = UpdateError {
            message: "Test error".to_string(),
        };
        assert_eq!(error.to_string(), "Update error: Test error");
    }

    #[test]
    fn test_multiple_commands_success() {
        let config = create_test_config_with_subcommands();
        let pm = PackageManager::new(config);

        let result = pm.check_with_subcommand("test", Some("multi"));
        assert!(result.is_ok());

        let result = pm.update_with_subcommand("test", Some("multi"));
        assert!(result.is_ok());
    }

    #[test]
    fn test_multiple_commands_failure() {
        let config = create_test_config_with_subcommands();
        let pm = PackageManager::new(config);

        let result = pm.check_with_subcommand("test", Some("fail"));
        assert!(result.is_err());

        let result = pm.update_with_subcommand("test", Some("fail"));
        assert!(result.is_err());
    }

    #[test]
    fn test_subcommand_execution() {
        let config = create_test_config_with_subcommands();
        let pm = PackageManager::new(config);

        // Test default subcommand
        let result = pm.check("test");
        assert!(result.is_ok());

        // Test specific subcommand
        let result = pm.check_with_subcommand("test", Some("multi"));
        assert!(result.is_ok());

        // Test unknown subcommand
        let result = pm.check_with_subcommand("test", Some("nonexistent"));
        assert!(result.is_err());

        // Test unknown package manager
        let result = pm.check_with_subcommand("nonexistent", None);
        assert!(result.is_err());
    }

    #[test]
    fn test_simple_format_compatibility() {
        let config = create_test_config_with_simple_format();
        let pm = PackageManager::new(config);
        
        // Test package manager with direct check/update fields only
        let result = pm.check("simple");
        assert!(result.is_ok());
        
        let result = pm.update("simple");
        assert!(result.is_ok());
        
        // Test error handling for failing direct check/update fields
        let result = pm.check("simple_fail");
        assert!(result.is_err());
        
        let result = pm.update("simple_fail");
        assert!(result.is_err());
        
        // Test behavior when both direct fields and subcommands are present
        // Default behavior: subcommands take priority
        let result = pm.check("mixed");
        assert!(result.is_ok());
        
        // Test with specific subcommand specified
        let result = pm.check_with_subcommand("mixed", Some("sub"));
        assert!(result.is_ok());
        
        // Test with nonexistent subcommand
        let result = pm.check_with_subcommand("mixed", Some("nonexistent"));
        assert!(result.is_err());
    }
}
