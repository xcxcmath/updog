use clap::Parser;
use std::collections::HashMap;
use std::process;
use tracing::{error, info};
use updog::{
    cli::{Cli, Commands},
    Config, PackageManager,
};

// Execution result tracking struct
struct ExecutionResult {
    success: bool,
    message: String,
}

fn main() {
    let cli = Cli::parse();

    // Initialize logging
    let level = if cli.verbose { "debug" } else { "info" };
    tracing_subscriber::fmt().with_env_filter(level).init();

    // Load configuration
    let config_path = cli.get_config_path();
    let config = match Config::from_file(&config_path) {
        Ok(config) => config,
        Err(e) => {
            error!("Failed to load config from {:?}: {}", config_path, e);
            info!("Using default configuration");
            Config::default()
        }
    };

    // Check if dry run mode is enabled
    let is_dry_run = cli.command.is_dry_run();
    if is_dry_run {
        info!("Dry run mode - no changes will be made");
    }

    let pm = PackageManager::with_dry_run(config, is_dry_run);

    // Set up cleanup on panic
    let pm_clone = std::panic::catch_unwind(|| {
        execute_command(&cli.command, &pm);
        pm // Return PackageManager instance on normal exit
    });

    // Perform cleanup before exiting
    if let Ok(pm) = pm_clone {
        info!("Cleaning up before exit");
        pm.cleanup();
    } else {
        error!("Program panicked! Attempting cleanup...");
        // Attempt cleanup on panic (create a new PackageManager instance)
        let fallback_pm = PackageManager::with_dry_run(Config::default(), true);
        fallback_pm.cleanup();
        process::exit(1);
    }
}

fn execute_command(command: &Commands, pm: &PackageManager) {
    match command {
        Commands::Check {
            package_manager, ..
        } => {
            // If specific package manager is provided, use it. Otherwise, use all available
            let execution_items = match package_manager {
                Some(_) => {
                    // Parse package manager to get manager name and subcommand
                    let parsed = command.parse_package_manager().unwrap();
                    vec![parsed]
                }
                None => {
                    // Use all package managers with their default subcommands
                    pm.config.commands.iter()
                        .map(|pm_config| (pm_config.id.clone(), None))
                        .collect()
                }
            };

            let mut has_error = false;
            let mut results = HashMap::new();

            for (manager_name, subcommand) in execution_items {
                // Display name for logs and results
                let display_name = match &subcommand {
                    Some(sc) => format!("{}:{}", manager_name, sc),
                    None => manager_name.clone(),
                };
                
                info!("Checking updates for {}", display_name);
                
                // Execute check command with the appropriate subcommand
                let result = match &subcommand {
                    Some(sc) => pm.check_with_subcommand(&manager_name, Some(sc)),
                    None => pm.check(&manager_name),
                };
                
                match result {
                    Ok(_) => {
                        results.insert(
                            display_name,
                            ExecutionResult {
                                success: true,
                                message: "Successfully checked for updates".to_string(),
                            },
                        );
                    }
                    Err(e) => {
                        error!("{}: {}", manager_name, e);
                        results.insert(
                            display_name,
                            ExecutionResult {
                                success: false,
                                message: format!("Error: {}", e),
                            },
                        );
                        has_error = true;
                    }
                }
            }

            // Print summary of execution result
            print_summary("Check", &results);

            if has_error {
                process::exit(1);
            }
        }

        Commands::Update {
            package_manager, ..
        } => {
            // If specific package manager is provided, use it. Otherwise, use all available
            let execution_items = match package_manager {
                Some(_) => {
                    // Parse package manager to get manager name and subcommand
                    let parsed = command.parse_package_manager().unwrap();
                    vec![parsed]
                }
                None => {
                    // Use all package managers with their default subcommands
                    pm.config.commands.iter()
                        .map(|pm_config| (pm_config.id.clone(), None))
                        .collect()
                }
            };

            let mut has_error = false;
            let mut results = HashMap::new();

            for (manager_name, subcommand) in execution_items {
                // Display name for logs and results
                let display_name = match &subcommand {
                    Some(sc) => format!("{}:{}", manager_name, sc),
                    None => manager_name.clone(),
                };
                
                info!("Updating {}", display_name);
                
                // Execute update command with the appropriate subcommand
                let result = match &subcommand {
                    Some(sc) => pm.update_with_subcommand(&manager_name, Some(sc)),
                    None => pm.update(&manager_name),
                };
                
                match result {
                    Ok(_) => {
                        results.insert(
                            display_name,
                            ExecutionResult {
                                success: true,
                                message: "Successfully updated".to_string(),
                            },
                        );
                    }
                    Err(e) => {
                        error!("{}: {}", manager_name, e);
                        results.insert(
                            display_name,
                            ExecutionResult {
                                success: false,
                                message: format!("Error: {}", e),
                            },
                        );
                        has_error = true;
                    }
                }
            }

            // Print summary of execution result
            print_summary("Update", &results);

            if has_error {
                process::exit(1);
            }
        }

        Commands::Tui => {
            info!("TUI mode not implemented yet");
        }
    }
}

// Print summary of execution result
fn print_summary(operation: &str, results: &HashMap<String, ExecutionResult>) {
    println!("\n{} Summary:", operation);
    println!("==============================================");

    let mut success_count = 0;
    let mut failure_count = 0;

    // Calculate successful items
    for (_, result) in results.iter() {
        if result.success {
            success_count += 1;
        } else {
            failure_count += 1;
        }
    }

    // Print successful items if there are any
    if success_count > 0 {
        println!("✅ Successful:");
        for (name, result) in results.iter() {
            if result.success {
                println!("  - {}: {}", name, result.message);
            }
        }
    }

    // Print failed items if there are any
    if failure_count > 0 {
        if success_count > 0 {
            println!(); // Add empty line between success and failure items
        }
        println!("❌ Failed:");
        for (name, result) in results.iter() {
            if !result.success {
                println!("  - {}: {}", name, result.message);
            }
        }
    }

    // Print statistics
    println!("\nTotal: {}, Successful: {}, Failed: {}", success_count + failure_count, success_count, failure_count);
}
