use crate::config::Config;
use std::collections::HashSet;
use std::error::Error;
use std::fmt;
use std::process::{Command, ExitStatus, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use tracing::{error, info, warn};

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
    process_tracker: Arc<Mutex<ProcessTracker>>,
}

// Structure to track running processes
struct ProcessTracker {
    active_processes: HashSet<u32>, // Set of active process IDs
    shutdown_requested: Arc<AtomicBool>,
}

impl ProcessTracker {
    fn new() -> Self {
        Self {
            active_processes: HashSet::new(),
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
                warn!(
                    "Process termination on Windows not implemented for PID: {}",
                    pid
                );
            }
        }
    }
}

impl PackageManager {
    fn run_command(&self, command: &str) -> Result<ExitStatus, UpdateError> {
        // Abort command execution if shutdown is requested
        {
            let tracker = self.process_tracker.lock().unwrap();
            if tracker.is_shutdown_requested() {
                return Err(UpdateError {
                    message: "Command execution aborted due to shutdown request".to_string(),
                });
            }
        }

        // Dry run mode - just log the command without executing it
        if self.dry_run {
            info!("[DRY RUN] Would execute: {}", command);
            // Instead of using from_raw, execute a simple "true" command
            // which will always succeed with exit code 0
            return Command::new("true").status().map_err(|e| UpdateError {
                message: format!("Failed to execute command: {}", e),
            });
        }

        // Use spawn() to execute the command - wait() for it later
        let mut child = Command::new("sh")
            .arg("-c")
            .arg(command)
            .stdin(Stdio::inherit())
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .spawn()
            .map_err(|e| UpdateError {
                message: format!("Failed to execute command: {}", e),
            })?;

        let pid = child.id();

        // Register process ID
        {
            let mut tracker = self.process_tracker.lock().unwrap();
            tracker.register_process(pid);
            info!("Started process {} for command: {}", pid, command);
        }

        // Wait for process completion
        let status = child.wait().map_err(|e| UpdateError {
            message: format!("Failed to wait for command completion: {}", e),
        })?;

        // Unregister process ID
        {
            let mut tracker = self.process_tracker.lock().unwrap();
            tracker.unregister_process(pid);
            info!("Process {} completed", pid);
        }

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
        let process_tracker = Arc::new(Mutex::new(ProcessTracker::new()));
        Self::setup_signal_handlers(Arc::clone(&process_tracker));

        Self {
            config,
            dry_run: false,
            process_tracker,
        }
    }

    pub fn with_dry_run(config: Config, dry_run: bool) -> Self {
        let process_tracker = Arc::new(Mutex::new(ProcessTracker::new()));
        Self::setup_signal_handlers(Arc::clone(&process_tracker));

        Self {
            config,
            dry_run,
            process_tracker,
        }
    }

    pub fn with_default_config() -> Self {
        let process_tracker = Arc::new(Mutex::new(ProcessTracker::new()));
        Self::setup_signal_handlers(Arc::clone(&process_tracker));

        Self {
            config: Config::default(),
            dry_run: false,
            process_tracker,
        }
    }

    // Set up signal handlers (Unix platforms only)
    #[cfg(unix)]
    fn setup_signal_handlers(process_tracker: Arc<Mutex<ProcessTracker>>) {
        use std::sync::Once;
        static INIT: Once = Once::new();

        INIT.call_once(|| {
            let tracker_clone = Arc::clone(&process_tracker);

            // SIGTERM (termination request) handler
            if let Err(e) = ctrlc::set_handler(move || {
                error!("Received termination signal, shutting down...");
                let tracker = tracker_clone.lock().unwrap();
                tracker.request_shutdown();
                tracker.terminate_all_processes();
                std::process::exit(130); // Exit due to signal
            }) {
                error!("Error setting up signal handler: {}", e);
            }
        });
    }

    // Simple implementation on Windows
    #[cfg(not(unix))]
    fn setup_signal_handlers(process_tracker: Arc<Mutex<ProcessTracker>>) {
        use std::sync::Once;
        static INIT: Once = Once::new();

        INIT.call_once(|| {
            let tracker_clone = Arc::clone(&process_tracker);

            if let Err(e) = ctrlc::set_handler(move || {
                error!("Received termination signal, shutting down...");
                let tracker = tracker_clone.lock().unwrap();
                tracker.request_shutdown();
                std::process::exit(130);
            }) {
                error!("Error setting up signal handler: {}", e);
            }
        });
    }

    // Clean up on exit
    pub fn cleanup(&self) {
        let tracker = self.process_tracker.lock().unwrap();
        tracker.request_shutdown();
        tracker.terminate_all_processes();
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

        assert!(pm.check("no_check").is_ok());
    }

    #[test]
    fn test_package_manager_no_update_command() {
        let config = create_test_config();
        let pm = PackageManager::new(config);

        assert!(pm.update("no_update").is_err());
    }

    #[test]
    fn test_failed_command() {
        let config = create_test_config();
        let pm = PackageManager::new(config);

        assert!(pm.check("fail").is_err());
        assert!(pm.update("fail").is_err());
    }

    #[test]
    fn test_unknown_package_manager() {
        let config = create_test_config();
        let pm = PackageManager::new(config);

        assert!(pm.check("unknown").is_err());
    }

    #[test]
    fn test_update_error_display() {
        let error = UpdateError {
            message: "Test error".to_string(),
        };
        assert_eq!(format!("{}", error), "Update error: Test error");
    }
}
